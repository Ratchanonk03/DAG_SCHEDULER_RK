//! Benchmark harness. (PROVIDED — you should not need to modify this,
//! but read it: it is how your submission will be measured.)

// The starter ships with unimplemented stubs, so the compiler would
// otherwise emit a wall of dead-code and unused-import warnings on a fresh
// clone. Once your implementation is complete, DELETE this line and make
// sure `cargo build` is still warning-free — that is part of the grade.
#![allow(dead_code, unused_imports, unused_variables)]

mod dag;
mod rng;
mod scheduler;
mod stats;
mod stealq;
mod tensor;

use dag::{
    build_attention_dag, build_chain_dag, build_diamond_dag, build_wide_fan_dag, ComputeDAG, OpKind,
};
use scheduler::{execute_sequential, Scheduler, SchedulerConfig};
use stats::RunStats;
use tensor::{Tensor, TensorStore};

/// Initialize Input nodes with deterministic tensors.
///
/// Seeding from the node ID means every run — sequential or parallel,
/// 1 worker or 8 — sees byte-identical inputs.
fn init_inputs(dag: &ComputeDAG, store: &TensorStore) {
    for node in &dag.nodes {
        if matches!(node.kind, OpKind::Input) {
            let (rows, cols) = node.output_shape;
            store.write(node.id, Tensor::seeded(rows, cols, node.id as u64 + 1));
        }
    }
}

/// Run a benchmark suite: sequential baseline + parallel at each worker count.
fn benchmark_dag(dag: &ComputeDAG, worker_counts: &[usize]) {
    println!("\n{}", "=".repeat(64));
    println!("Benchmarking: {}", dag.name);
    println!(
        "Nodes: {}   Critical path: {} nodes",
        dag.len(),
        dag.critical_path_len()
    );
    println!("{}", "=".repeat(64));

    // Sequential reference. This is NOT a row in the table — it exists so you
    // can quote scheduler overhead at 1 worker, which is the honest way to
    // separate "Amdahl's Law limited me" from "my scheduler is expensive".
    let seq_store = TensorStore::new(dag.len());
    init_inputs(dag, &seq_store);
    let seq_time = execute_sequential(dag, &seq_store);
    println!(
        "\nSequential reference: {:.1}ms",
        seq_time.as_secs_f64() * 1000.0
    );

    // Discarded warm-up run. The 1-worker row is measured first, so without
    // this it absorbs all the cold-start cost — first-touch page faults, cold
    // caches, lazy thread-stack allocation — and every speedup after it comes
    // out inflated. On small DAGs that artefact can exceed the real effect.
    {
        let warm_store = TensorStore::new(dag.len());
        init_inputs(dag, &warm_store);
        let _ = Scheduler::new(2, SchedulerConfig::default()).run(dag, &warm_store);
    }

    // Every row in the table is a real scheduler run, including 1 worker.
    let mut all_stats = vec![];
    for &num_workers in worker_counts {
        let par_store = TensorStore::new(dag.len());
        init_inputs(dag, &par_store);

        let sched = Scheduler::new(num_workers, SchedulerConfig::default());
        let stats = sched.run(dag, &par_store);
        stats.report();
        all_stats.push(stats);
    }

    println!("--- Results (baseline = 1 worker) ---");
    RunStats::compare(&all_stats);

    if let Some(one) = all_stats.first() {
        let overhead = one.total_time.as_secs_f64() / seq_time.as_secs_f64().max(1e-12);
        println!(
            "Scheduler overhead at 1 worker: {:.2}x sequential",
            overhead
        );
    }
}

/// Parse a `--flag value` pair from argv.
fn arg<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> T {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // --size  : tensor dimension for the benchmarks (default 64)
    // --heads : attention heads in the transformer DAG (default 4)
    let size: usize = arg(&args, "--size", 64);
    let heads: usize = arg(&args, "--heads", 4);

    let worker_counts = vec![1, 2, 4, 8];

    println!("╔════════════════════════════════════════════════════╗");
    println!("║   Work-Stealing DAG Scheduler — Benchmarks         ║");
    println!("╚════════════════════════════════════════════════════╝");
    println!("Tensor size : {}x{}", size, size);
    println!("Heads       : {}", heads);
    println!("Workers     : {:?}", worker_counts);

    // --- Diamond DAG (basic fan-out / fan-in) ---
    benchmark_dag(&build_diamond_dag(size, size), &worker_counts);

    // --- Wide-Fan DAG (maximum parallelism, steal throughput) ---
    benchmark_dag(&build_wide_fan_dag(12, size, size), &worker_counts);

    // --- Chain DAG (zero parallelism: pure scheduler overhead) ---
    benchmark_dag(&build_chain_dag(24, size, size), &worker_counts);

    // --- Attention Layer DAG (realistic mixed parallelism) ---
    benchmark_dag(&build_attention_dag(size, size, heads), &worker_counts);

    println!("\nAll benchmarks complete.");
}
