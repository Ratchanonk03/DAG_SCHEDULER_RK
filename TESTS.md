| Test | Property validated |
| --- | --- |
| `dag::test_diamond_frontier` | Only the root is runnable initially; both independent branches become runnable once it finishes. This prevents tasks from running before their prerequisites. |
| `dag::test_diamond_execution_order` | The join task runs only after both branches complete, preserving dependency correctness in fork/join workloads. |
| `dag::test_chain_has_no_parallelism` | A linear chain exposes one runnable task at a time, distinguishing genuine scheduler parallelism from impossible parallelism. |
| `dag::test_retire_appends_without_clearing` | Completing a task adds newly unblocked work without losing tasks already in the ready queue, preventing ready work from being dropped. |
| `dag::test_attention_dag_frontier` | The representative attention workload produces the correct ready set, checking realistic dependency structure rather than only toy graphs. |
| `dag::test_wide_fan_frontier` | All independent fan-out tasks are released together, showing that the DAG exposes available concurrency. |
| `dag::test_attention_dag_node_count` | Graph construction creates the expected number of computation nodes, catching omissions and duplicates. |
| `dag::test_attention_critical_path_is_short` | The graph keeps its intended short critical path, so unnecessary edges do not reduce available parallelism. |
| `dag::test_wide_fan_parallelism` | A wide graph reports parallel work, confirming that the scheduler input has exploitable concurrency. |
| `rng::test_below_in_range` | Integer samples never exceed the requested bound, preventing invalid worker or task indexes. |
| `rng::test_deterministic` | A fixed seed produces the same sequence, making randomized experiments and failures reproducible. |
| `rng::test_f32_in_range` | Floating-point samples stay within the promised interval, preventing invalid probability or timing values. |
| `scheduler::test_chain_correctness` | Every task in a chain completes in prerequisite order, providing the baseline execution-correctness case. |
| `scheduler::test_more_workers_than_tasks` | Idle extra workers do not cause duplicate execution or failures, covering sparse workloads on larger worker pools. |
| `scheduler::test_stats_are_populated` | Execution records meaningful statistics, allowing benchmark results to be reported and interpreted. |
| `scheduler::test_diamond_correctness` | Parallel branches run after the root, and the join waits for both, testing the core dependency-release path. |
| `scheduler::test_wide_fan_correctness` | All independent tasks and their final join complete correctly, testing execution across many runnable tasks. |
| `scheduler::test_single_worker_matches_sequential` | One-worker scheduling produces the same result as sequential execution, establishing correctness independent of parallelism. |
| `scheduler::test_attention_correctness` | The scheduler completes the representative attention DAG, combining graph resolution, queues, and worker execution. |
| `stealq::test_opposite_ends` | The owner removes local work from one end while thieves take from the other, preserving the intended locality and stealing policy. |
| `stealq::test_push_pop_interleaved` | Alternating owner pushes and pops preserve correct queue state, catching bookkeeping errors during local execution. |
| `stealq::test_concurrent_steal` | Simultaneous thieves safely claim work without races, duplicate tasks, or queue corruption. |
| `stealq::test_push_pop_lifo` | Local pops use LIFO order, improving locality and confirming that owner behavior differs from stealing behavior. |
| `stealq::test_steal_empty` | A thief safely receives no work when the queue is empty, preventing errors during idle stealing attempts. |
| `stealq::test_steal_fifo` | Stolen tasks use FIFO order, protecting the queue's owner/thief ordering contract. |
| `stealq::test_owner_and_thieves` | Local execution and remote stealing coexist without losing or duplicating tasks. |
| `stealq::test_stress_many_thieves` | Correctness holds under high contention from many thieves, exercising the synchronization path used in parallel runs. |