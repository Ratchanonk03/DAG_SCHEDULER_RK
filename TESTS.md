| Test | Property validated |
| --- | --- |
| `dag::test_diamond_frontier` | A diamond DAG exposes the correct ready-task frontier. |
| `dag::test_diamond_execution_order` | Diamond dependencies produce a valid execution order. |
| `dag::test_chain_has_no_parallelism` | A linear dependency chain has no parallel work. |
| `dag::test_retire_appends_without_clearing` | Retiring work appends newly ready tasks without discarding existing ready tasks. |
| `dag::test_attention_dag_frontier` | The attention-style DAG computes its ready frontier correctly. |
| `dag::test_wide_fan_frontier` | A wide fan-out/fan-in DAG exposes the expected frontier. |
| `dag::test_attention_dag_node_count` | The attention-style DAG contains the expected number of nodes. |
| `dag::test_attention_critical_path_is_short` | The attention-style DAG has the intended short critical path. |
| `dag::test_wide_fan_parallelism` | A wide DAG provides parallel execution opportunities. |
| `rng::test_below_in_range` | Bounded random values remain within the requested range. |
| `rng::test_deterministic` | The RNG produces repeatable output for a fixed seed. |
| `rng::test_f32_in_range` | Generated floating-point random values remain in range. |
| `scheduler::test_chain_correctness` | The scheduler completes a dependency chain correctly. |
| `scheduler::test_more_workers_than_tasks` | Scheduling remains correct when workers outnumber tasks. |
| `scheduler::test_stats_are_populated` | Scheduler statistics are collected and populated. |
| `scheduler::test_diamond_correctness` | The scheduler respects diamond-shaped dependencies. |
| `scheduler::test_wide_fan_correctness` | The scheduler completes a wide fan-out/fan-in graph correctly. |
| `scheduler::test_single_worker_matches_sequential` | One-worker execution matches sequential behavior. |
| `scheduler::test_attention_correctness` | The scheduler completes the attention-style DAG correctly. |
| `stealq::test_opposite_ends` | Owner and thief operate on opposite ends of the queue. |
| `stealq::test_push_pop_interleaved` | Interleaved owner pushes and pops preserve queue behavior. |
| `stealq::test_concurrent_steal` | Concurrent stealing behaves safely and correctly. |
| `stealq::test_push_pop_lifo` | Owner pop operations follow LIFO order. |
| `stealq::test_steal_empty` | Stealing from an empty queue is handled safely. |
| `stealq::test_steal_fifo` | Thieves steal tasks in FIFO order. |
| `stealq::test_owner_and_thieves` | An owner and multiple thieves coordinate correctly. |
| `stealq::test_stress_many_thieves` | The queue remains correct under many concurrent thieves. |