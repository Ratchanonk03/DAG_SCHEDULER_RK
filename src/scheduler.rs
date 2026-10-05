//! Part C — the work-stealing scheduler.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::dag::{ComputeDAG, OpKind, ReadyTracker};
use crate::stealq::{StealOutcome, StealQueue};
use crate::rng::SplitMix64;
use crate::stats::RunStats;
use crate::tensor::{self, TensorStore};

/// Execute a single DAG node: read inputs from the store, compute,
/// write the output back to the store. (PROVIDED.)
pub fn execute_node(node_id: usize, dag: &ComputeDAG, store: &TensorStore) {
    let node = &dag.nodes[node_id];

    let output = match &node.kind {
        OpKind::Input => {
            // Input nodes are pre-populated before the scheduler starts.
            return;
        }
        OpKind::MatMul => {
            let a = store.read(node.deps[0]);
            let b = store.read(node.deps[1]);
            tensor::matmul(&a, &b)
        }
        OpKind::ElemAdd => {
            let a = store.read(node.deps[0]);
            let b = store.read(node.deps[1]);
            tensor::elem_add(&a, &b)
        }
        OpKind::ReLU => {
            let a = store.read(node.deps[0]);
            tensor::relu(&a)
        }
        OpKind::Softmax => {
            let a = store.read(node.deps[0]);
            tensor::softmax(&a)
        }
        OpKind::Transpose => {
            let a = store.read(node.deps[0]);
            tensor::transpose(&a)
        }
        OpKind::Concat { axis } => {
            let a = store.read(node.deps[0]);
            let b = store.read(node.deps[1]);
            tensor::concat(&a, &b, *axis)
        }
        OpKind::LayerNorm => {
            let a = store.read(node.deps[0]);
            tensor::layer_norm(&a)
        }
    };

    store.write(node_id, output);
}

/// Execute the DAG sequentially (single-threaded, topological order).
///
/// This is the **reference implementation** for correctness validation:
/// your parallel scheduler must produce the same final tensor. (PROVIDED.)
pub fn execute_sequential(dag: &ComputeDAG, store: &TensorStore) -> Duration {
    let tracker = ReadyTracker::new(dag);
    let start = Instant::now();

    let mut ready: Vec<usize> = tracker.initial_frontier();
    let mut newly_ready: Vec<usize> = Vec::new();

    while !ready.is_empty() {
        newly_ready.clear();
        for &node_id in &ready {
            execute_node(node_id, dag, store);
            tracker.retire(node_id, &mut newly_ready);
        }
        std::mem::swap(&mut ready, &mut newly_ready);
    }

    assert!(
        tracker.is_drained(),
        "Sequential execution did not complete all nodes"
    );
    start.elapsed()
}

/// Tunable scheduler parameters.
///
/// Pulling these out of the worker loop lets you sweep them in your
/// benchmarks instead of recompiling — useful for the writeup, where you
/// are asked to justify your backoff choice rather than just assert it.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerConfig {
    /// Initial backoff after a fully failed steal round.
    pub min_backoff: Duration,
    /// Ceiling for exponential backoff.
    pub max_backoff: Duration,
    /// How many victims to probe before giving up and backing off.
    pub steal_tries_per_round: usize,
    /// Base seed for per-worker victim selection (keeps runs reproducible).
    pub seed: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            min_backoff: Duration::from_micros(2),
            max_backoff: Duration::from_micros(200),
            steal_tries_per_round: 4,
            seed: 0x5EED_0DA6_u64,
        }
    }
}

/// The work-stealing DAG scheduler.
pub struct Scheduler {
    num_workers: usize,
    config: SchedulerConfig,
}

impl Scheduler {
    /// Create a scheduler with `num_workers` threads and the given config.
    ///
    /// Use `SchedulerConfig::default()` unless you are sweeping parameters.
    pub fn new(num_workers: usize, config: SchedulerConfig) -> Self {
        assert!(num_workers > 0, "need at least one worker");
        Self {
            num_workers,
            config,
        }
    }

    /// Execute the DAG using work-stealing parallelism.
    ///
    /// Returns execution statistics — see `RunStats`. Note that you must
    /// populate `steal_attempts_per_worker` as well as `steals_per_worker`:
    /// the ratio between them is what tells you whether stealing is paying
    /// for itself.
    pub fn run(&self, dag: &ComputeDAG, store: &TensorStore) -> RunStats {
        let num_workers = self.num_workers;
        let config = self.config;
        let tracker = Arc::new(ReadyTracker::new(dag));

        // TODO(DONE): Implement the work-stealing scheduler.
        //
        // HIGH-LEVEL STEPS
        //
        // 1. Create one StealQueue<usize> per worker (queues hold node IDs).
        //    Wrap each in Arc so every thread can reach every queue.
        //
        //      let queues: Vec<Arc<StealQueue<usize>>> = (0..num_workers)
        //          .map(|_| Arc::new(StealQueue::with_capacity(dag.len())))
        //          .collect();
        //
        // 2. Seed the initial frontier round-robin across the queues:
        //
        //      for (i, node_id) in tracker.initial_frontier().into_iter().enumerate() {
        //          queues[i % num_workers].push(node_id);
        //      }
        //
        // 3. Spawn num_workers threads. Each worker owns:
        //      - its worker id
        //      - a SplitMix64 seeded from config.seed and its id
        //      - one reusable Vec<usize> scratch buffer for `retire`
        //      - counters: tasks, steals, steal_attempts, idle_time
        //      - a current backoff, starting at config.min_backoff
        //
        //    Worker loop:
        //
        //      loop {
        //          // Fast path: take from my own queue.
        //          let mut task = my_queue.pop();
        //
        //          // Slow path: probe up to config.steal_tries_per_round victims.
        //          if task.is_none() {
        //              for _ in 0..config.steal_tries_per_round {
        //                  let victim = rng.below(num_workers);
        //                  if victim == my_id { continue; }   // never steal from self
        //                  steal_attempts += 1;                // count EVERY probe
        //                  match queues[victim].steal() {
        //                      StealOutcome::Took(t)     => { steals += 1; task = Some(t); break; }
        //                      StealOutcome::Contended   => { /* someone beat me; try again */ }
        //                      StealOutcome::Vacant      => { /* nothing there; next victim */ }
        //                  }
        //              }
        //          }
        //
        //          match task {
        //              Some(node_id) => {
        //                  execute_node(node_id, dag, store);
        //                  tasks += 1;
        //
        //                  scratch.clear();
        //                  tracker.retire(node_id, &mut scratch);
        //                  for &succ in &scratch { my_queue.push(succ); }
        //
        //                  backoff = config.min_backoff;   // reset on success
        //              }
        //              None => {
        //                  if tracker.is_drained() { break; }   // all work done
        //
        //                  let t0 = Instant::now();
        //                  thread::sleep(backoff);
        //                  idle_time += t0.elapsed();
        //                  backoff = (backoff * 2).min(config.max_backoff);
        //              }
        //          }
        //      }
        //
        // 4. Join every worker and collect its counters.
        //
        // 5. Build and return RunStats — including steal_attempts_per_worker.
        //
        // THREADING NOTE
        //   `dag` and `store` are borrowed, not owned, so plain
        //   `thread::spawn` will not accept them. Use
        //   `thread::scope(|s| { ... s.spawn(...) ... })` (stable since
        //   Rust 1.63), which guarantees the threads finish before the
        //   borrows end. This is the clean solution — do not reach for
        //   `unsafe` or leak an Arc to get around it.

        let start = Instant::now();

        // --- YOUR CODE HERE ---

        let queues = (0..num_workers)
            .map(|_| Arc::new(StealQueue::<usize>::with_capacity(dag.len())))
            .collect::<Vec<_>>();

        for (i, node_id) in tracker.initial_frontier().into_iter().enumerate() {
            queues[i % num_workers].push(node_id);
        }
        
        let all_queues = &queues;

        let worker_results = thread::scope(|scope| {
            let mut handles = Vec::new();
            
            for worker_id in 0..num_workers {
                let tracker_handle = Arc::clone(&tracker);

                handles.push(scope.spawn(move || {
                    let my_queue = Arc::clone(&all_queues[worker_id]);
                    
                    let mut rng = SplitMix64::new(config.seed.wrapping_add(worker_id as u64));
                    let mut scratch: Vec<usize> = Vec::new();
                    let mut backoff = config.min_backoff;

                    let mut tasks = 0usize;
                    let mut steals = 0usize;
                    let mut steal_attempts = 0usize;
                    let mut idle_time = Duration::ZERO;


                     loop {
                        // Fast path: take from my own queue.
                        let mut task = my_queue.pop();
                
                        // Slow path: probe up to config.steal_tries_per_round victims.
                        if task.is_none() {
                            for _ in 0..config.steal_tries_per_round {
                                let victim = rng.below(num_workers);
                                if victim == worker_id { continue; }   // never steal from self
                                steal_attempts += 1;                // count EVERY probe
                                match &all_queues[victim].steal() {
                                    StealOutcome::Took(t)     => { steals += 1; task = Some(*t); break; }
                                    StealOutcome::Contended   => { /* someone beat me; try again */ }
                                    StealOutcome::Vacant      => { /* nothing there; next victim */ }
                                }
                            }
                        }
                
                        match task {
                            Some(node_id) => {
                                execute_node(node_id, dag, store);
                                tasks += 1;
                
                                scratch.clear();
                                tracker_handle.retire(node_id, &mut scratch);
                                for &succ in &scratch { my_queue.push(succ); }
                
                                backoff = config.min_backoff;   // reset on success
                            }
                            None => {
                                if tracker_handle.is_drained() { break; }   // all work done
                
                                let t0 = Instant::now();
                                thread::sleep(backoff);
                                idle_time += t0.elapsed();
                                backoff = (backoff * 2).min(config.max_backoff);
                            }
                        }
                    }

                    (tasks, steals, steal_attempts, idle_time)
                }));
            }

            handles.into_iter().map(|handle| handle.join().unwrap()).collect::<Vec<_>>()
        });

        let total_time = start.elapsed();

        // Stats collection
        let mut stats = RunStats::new(dag.name.clone(), num_workers);
        stats.total_time = total_time;

        for (worker_id, (tasks, steals, steal_attempts, idle_time)) in worker_results.into_iter().enumerate(){
            stats.tasks_per_worker[worker_id] = tasks;
            stats.steals_per_worker[worker_id] = steals;
            stats.steal_attempts_per_worker[worker_id] = steal_attempts;
            stats.idle_time_per_worker[worker_id] = idle_time;
        }

        stats.total_tasks = dag.len();
        stats

        // --- END YOUR CODE ---
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{build_attention_dag, build_chain_dag, build_diamond_dag, build_wide_fan_dag};
    use crate::tensor::Tensor;

    /// Initialize Input nodes with deterministic tensors.
    fn init_inputs(dag: &ComputeDAG, store: &TensorStore) {
        for node in &dag.nodes {
            if matches!(node.kind, OpKind::Input) {
                let (rows, cols) = node.output_shape;
                store.write(node.id, Tensor::seeded(rows, cols, node.id as u64 + 1));
            }
        }
    }

    fn last_node(dag: &ComputeDAG) -> usize {
        dag.len() - 1
    }

    /// Verify that parallel execution matches sequential execution.
    fn verify_correctness(dag: &ComputeDAG, num_workers: usize) {
        let store_seq = TensorStore::new(dag.len());
        init_inputs(dag, &store_seq);
        execute_sequential(dag, &store_seq);
        let expected = store_seq.read(last_node(dag));

        let store_par = TensorStore::new(dag.len());
        init_inputs(dag, &store_par);

        let scheduler = Scheduler::new(num_workers, SchedulerConfig::default());
        let stats = scheduler.run(dag, &store_par);
        let actual = store_par.read(last_node(dag));

        assert_eq!(expected.rows, actual.rows);
        assert_eq!(expected.cols, actual.cols);
        for i in 0..expected.data.len() {
            let diff = (expected.data[i] - actual.data[i]).abs();
            assert!(
                diff < 1e-4,
                "Output mismatch at index {}: expected {}, got {} (diff={})",
                i,
                expected.data[i],
                actual.data[i],
                diff
            );
        }

        // Every node must have been executed exactly once, in total.
        let executed: usize = stats.tasks_per_worker.iter().sum();
        assert_eq!(
            executed,
            dag.len(),
            "every node must be executed exactly once"
        );
    }

    #[test]
    fn test_diamond_correctness() {
        let dag = build_diamond_dag(16, 16);
        verify_correctness(&dag, 2);
        verify_correctness(&dag, 4);
    }

    #[test]
    fn test_wide_fan_correctness() {
        let dag = build_wide_fan_dag(12, 16, 16);
        verify_correctness(&dag, 2);
        verify_correctness(&dag, 4);
    }

    #[test]
    fn test_chain_correctness() {
        // No parallelism available — the scheduler must still terminate
        // cleanly and must not spin.
        let dag = build_chain_dag(12, 16, 16);
        verify_correctness(&dag, 4);
    }

    #[test]
    fn test_attention_correctness() {
        let dag = build_attention_dag(8, 16, 4);
        verify_correctness(&dag, 2);
        verify_correctness(&dag, 4);
    }

    #[test]
    fn test_more_workers_than_tasks() {
        // 8 workers, 4 nodes: most workers find nothing and must exit
        // cleanly rather than hang or spin.
        let dag = build_diamond_dag(8, 8);
        verify_correctness(&dag, 8);
    }

    #[test]
    fn test_single_worker_matches_sequential() {
        let dag = build_attention_dag(8, 16, 2);
        verify_correctness(&dag, 1);
    }

    #[test]
    fn test_stats_are_populated() {
        let dag = build_wide_fan_dag(12, 16, 16);
        let store = TensorStore::new(dag.len());
        init_inputs(&dag, &store);

        let sched = Scheduler::new(4, SchedulerConfig::default());
        let stats = sched.run(&dag, &store);

        assert_eq!(stats.num_workers, 4);
        assert_eq!(stats.total_tasks, dag.len());
        assert_eq!(stats.tasks_per_worker.len(), 4);
        assert_eq!(stats.steals_per_worker.len(), 4);
        assert_eq!(stats.steal_attempts_per_worker.len(), 4);
        assert!(
            stats.total_steal_attempts() >= stats.total_steals(),
            "attempts must be at least the number of successes"
        );
        assert!(stats.total_time > Duration::ZERO);
    }
}
