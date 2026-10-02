//! Run statistics collection and reporting. (PROVIDED — reporting is
//! written for you; you are responsible for *populating* the fields.)

use std::time::Duration;

/// Statistics collected during a DAG execution run.
#[derive(Debug, Clone)]
pub struct RunStats {
    /// Name of the DAG that was executed.
    pub dag_name: String,
    /// Number of worker threads.
    pub num_workers: usize,
    /// Total wall-clock time for the execution.
    pub total_time: Duration,
    /// Number of tasks each worker executed.
    pub tasks_per_worker: Vec<usize>,
    /// Number of **successful** steals per worker.
    pub steals_per_worker: Vec<usize>,
    /// Number of steal *attempts* per worker (successful + failed).
    ///
    /// A failed attempt is one that returned `Vacant` or `Contended`.
    /// The ratio of successes to attempts tells you how much CPU your
    /// workers burn hunting for work that isn't there.
    pub steal_attempts_per_worker: Vec<usize>,
    /// Time each worker spent idle (backing off, failing to find work).
    pub idle_time_per_worker: Vec<Duration>,
    /// Total number of tasks executed.
    pub total_tasks: usize,
}

impl RunStats {
    /// Create an empty stats struct for N workers.
    pub fn new(dag_name: String, num_workers: usize) -> Self {
        Self {
            dag_name,
            num_workers,
            total_time: Duration::ZERO,
            tasks_per_worker: vec![0; num_workers],
            steals_per_worker: vec![0; num_workers],
            steal_attempts_per_worker: vec![0; num_workers],
            idle_time_per_worker: vec![Duration::ZERO; num_workers],
            total_tasks: 0,
        }
    }

    /// Total successful steals across all workers.
    pub fn total_steals(&self) -> usize {
        self.steals_per_worker.iter().sum()
    }

    /// Total steal attempts across all workers.
    pub fn total_steal_attempts(&self) -> usize {
        self.steal_attempts_per_worker.iter().sum()
    }

    /// Steal rate: successful steals as a fraction of tasks executed.
    ///
    /// "What share of the work moved between workers?"
    pub fn steal_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            0.0
        } else {
            self.total_steals() as f64 / self.total_tasks as f64
        }
    }

    /// Steal efficiency: successful steals as a fraction of steal attempts.
    ///
    /// "How often did hunting for work actually pay off?"
    /// Low efficiency means workers are burning cycles on failed probes.
    pub fn steal_efficiency(&self) -> f64 {
        let attempts = self.total_steal_attempts();
        if attempts == 0 {
            0.0
        } else {
            self.total_steals() as f64 / attempts as f64
        }
    }

    /// Load imbalance: `max_worker_load / mean_worker_load`.
    ///
    /// 1.0 is perfectly balanced. Higher means one worker did
    /// disproportionately more work than the others.
    pub fn load_imbalance(&self) -> f64 {
        if self.tasks_per_worker.is_empty() {
            return 0.0;
        }
        let total: usize = self.tasks_per_worker.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let mean = total as f64 / self.tasks_per_worker.len() as f64;
        let max = *self.tasks_per_worker.iter().max().unwrap() as f64;
        max / mean
    }

    /// Print a formatted report for a single run.
    pub fn report(&self) {
        println!();
        println!("===== DAG: {} =====", self.dag_name);
        println!("Workers:         {}", self.num_workers);
        println!(
            "Total time:      {:.1}ms",
            self.total_time.as_secs_f64() * 1000.0
        );
        println!("Tasks executed:  {}", self.total_tasks);
        println!(
            "Steals:          {} of {} attempts ({:.1}% of tasks, {:.1}% efficient)",
            self.total_steals(),
            self.total_steal_attempts(),
            self.steal_rate() * 100.0,
            self.steal_efficiency() * 100.0
        );
        println!("Worker load:     {:?}", self.tasks_per_worker);
        println!("Load imbalance:  {:.2}x", self.load_imbalance());
        println!(
            "Worker idle:     [{}]",
            self.idle_time_per_worker
                .iter()
                .map(|d| format!("{:.1}ms", d.as_secs_f64() * 1000.0))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("{}", "=".repeat(30 + self.dag_name.len()));
    }

    /// Print the comparison table required by the writeup.
    ///
    /// Columns are exactly the four the assignment asks for: wall-clock time,
    /// speedup, steal rate, and load distribution. The first entry in `runs`
    /// is the 1-worker baseline that speedup is measured against.
    pub fn compare(runs: &[RunStats]) {
        if runs.is_empty() {
            return;
        }

        let baseline = runs[0].total_time.as_secs_f64();

        println!();
        println!(
            "{:<9} {:>11} {:>9} {:>11}  {}",
            "Workers", "Time (ms)", "Speedup", "Steal Rate", "Load Distribution"
        );
        println!("{}", "-".repeat(78));

        for run in runs {
            let time_ms = run.total_time.as_secs_f64() * 1000.0;
            let speedup = if run.total_time.as_secs_f64() > 0.0 {
                baseline / run.total_time.as_secs_f64()
            } else {
                0.0
            };
            let load = run
                .tasks_per_worker
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            println!(
                "{:<9} {:>11.1} {:>8.2}x {:>10.1}%  [{}]",
                run.num_workers,
                time_ms,
                speedup,
                run.steal_rate() * 100.0,
                load
            );
        }
        println!();
    }
}
