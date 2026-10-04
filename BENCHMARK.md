╔════════════════════════════════════════════════════╗
║   Work-Stealing DAG Scheduler — Benchmarks         ║
╚════════════════════════════════════════════════════╝
Tensor size : 1024x1024
Heads       : 128
Workers     : [1, 2, 4, 8]

================================================================
Benchmarking: Diamond (1024x1024)
Nodes: 4   Critical path: 3 nodes
================================================================

Sequential reference: 2.4ms

===== DAG: Diamond (1024x1024) =====
Workers:         1
Total time:      1.2ms
Tasks executed:  4
Steals:          0 of 0 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [4]
Load imbalance:  1.00x
Worker idle:     [0.0ms]
=================================================

===== DAG: Diamond (1024x1024) =====
Workers:         2
Total time:      1.0ms
Tasks executed:  4
Steals:          1 of 22 attempts (25.0% of tasks, 4.5% efficient)
Worker load:     [3, 1]
Load imbalance:  1.50x
Worker idle:     [0.0ms, 0.6ms]
=================================================

===== DAG: Diamond (1024x1024) =====
Workers:         4
Total time:      1.0ms
Tasks executed:  4
Steals:          1 of 86 attempts (25.0% of tasks, 1.2% efficient)
Worker load:     [2, 2, 0, 0]
Load imbalance:  2.00x
Worker idle:     [0.3ms, 0.0ms, 0.9ms, 0.9ms]
=================================================

===== DAG: Diamond (1024x1024) =====
Workers:         8
Total time:      1.0ms
Tasks executed:  4
Steals:          1 of 239 attempts (25.0% of tasks, 0.4% efficient)
Worker load:     [2, 0, 0, 2, 0, 0, 0, 0]
Load imbalance:  4.00x
Worker idle:     [0.3ms, 0.9ms, 0.9ms, 0.0ms, 0.9ms, 0.9ms, 0.6ms, 0.6ms]
=================================================
--- Results (baseline = 1 worker) ---

Workers     Time (ms)   Speedup  Steal Rate  Load Distribution
------------------------------------------------------------------------------
1                 1.2     1.00x        0.0%  [4]
2                 1.0     1.15x       25.0%  [3, 1]
4                 1.0     1.22x       25.0%  [2, 2, 0, 0]
8                 1.0     1.14x       25.0%  [2, 0, 0, 2, 0, 0, 0, 0]

Scheduler overhead at 1 worker: 0.49x sequential

================================================================
Benchmarking: Wide-Fan (width=12, 1024x1024)
Nodes: 24   Critical path: 13 nodes
================================================================

Sequential reference: 11.7ms

===== DAG: Wide-Fan (width=12, 1024x1024) =====
Workers:         1
Total time:      9.7ms
Tasks executed:  24
Steals:          0 of 0 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [24]
Load imbalance:  1.00x
Worker idle:     [0.0ms]
============================================================

===== DAG: Wide-Fan (width=12, 1024x1024) =====
Workers:         2
Total time:      6.2ms
Tasks executed:  24
Steals:          4 of 42 attempts (16.7% of tasks, 9.5% efficient)
Worker load:     [9, 15]
Load imbalance:  1.25x
Worker idle:     [3.2ms, 0.0ms]
============================================================

===== DAG: Wide-Fan (width=12, 1024x1024) =====
Workers:         4
Total time:      5.4ms
Tasks executed:  24
Steals:          9 of 202 attempts (37.5% of tasks, 4.5% efficient)
Worker load:     [4, 4, 4, 12]
Load imbalance:  2.00x
Worker idle:     [3.4ms, 3.2ms, 3.2ms, 0.0ms]
============================================================

===== DAG: Wide-Fan (width=12, 1024x1024) =====
Workers:         8
Total time:      6.1ms
Tasks executed:  24
Steals:          10 of 568 attempts (41.7% of tasks, 1.8% efficient)
Worker load:     [3, 1, 2, 2, 12, 2, 1, 1]
Load imbalance:  4.00x
Worker idle:     [3.5ms, 4.1ms, 3.5ms, 3.5ms, 0.0ms, 3.5ms, 4.1ms, 4.2ms]
============================================================
--- Results (baseline = 1 worker) ---

Workers     Time (ms)   Speedup  Steal Rate  Load Distribution
------------------------------------------------------------------------------
1                 9.7     1.00x        0.0%  [24]
2                 6.2     1.58x       16.7%  [9, 15]
4                 5.4     1.81x       37.5%  [4, 4, 4, 12]
8                 6.1     1.60x       41.7%  [3, 1, 2, 2, 12, 2, 1, 1]

Scheduler overhead at 1 worker: 0.83x sequential

================================================================
Benchmarking: Chain (depth=24, 1024x1024)
Nodes: 25   Critical path: 25 nodes
================================================================

Sequential reference: 30.3ms

===== DAG: Chain (depth=24, 1024x1024) =====
Workers:         1
Total time:      26.8ms
Tasks executed:  25
Steals:          0 of 0 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [25]
Load imbalance:  1.00x
Worker idle:     [0.0ms]
=========================================================

===== DAG: Chain (depth=24, 1024x1024) =====
Workers:         2
Total time:      27.4ms
Tasks executed:  25
Steals:          0 of 237 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [25, 0]
Load imbalance:  2.00x
Worker idle:     [0.0ms, 27.4ms]
=========================================================

===== DAG: Chain (depth=24, 1024x1024) =====
Workers:         4
Total time:      27.9ms
Tasks executed:  25
Steals:          0 of 1046 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [25, 0, 0, 0]
Load imbalance:  4.00x
Worker idle:     [0.0ms, 27.8ms, 27.8ms, 27.8ms]
=========================================================

===== DAG: Chain (depth=24, 1024x1024) =====
Workers:         8
Total time:      27.9ms
Tasks executed:  25
Steals:          0 of 2798 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [25, 0, 0, 0, 0, 0, 0, 0]
Load imbalance:  8.00x
Worker idle:     [0.0ms, 27.8ms, 27.8ms, 27.8ms, 27.7ms, 27.8ms, 27.8ms, 27.7ms]
=========================================================
--- Results (baseline = 1 worker) ---

Workers     Time (ms)   Speedup  Steal Rate  Load Distribution
------------------------------------------------------------------------------
1                26.8     1.00x        0.0%  [25]
2                27.4     0.98x        0.0%  [25, 0]
4                27.9     0.96x        0.0%  [25, 0, 0, 0]
8                27.9     0.96x        0.0%  [25, 0, 0, 0, 0, 0, 0, 0]

Scheduler overhead at 1 worker: 0.88x sequential

================================================================
Benchmarking: Attention Layer (128-head, 1024x1024)
Nodes: 1419   Critical path: 141 nodes
================================================================

Sequential reference: 4799.1ms

===== DAG: Attention Layer (128-head, 1024x1024) =====
Workers:         1
Total time:      4490.8ms
Tasks executed:  1419
Steals:          0 of 0 attempts (0.0% of tasks, 0.0% efficient)
Worker load:     [1419]
Load imbalance:  1.00x
Worker idle:     [0.0ms]
===================================================================

===== DAG: Attention Layer (128-head, 1024x1024) =====
Workers:         2
Total time:      4857.5ms
Tasks executed:  1419
Steals:          54 of 9804 attempts (3.8% of tasks, 0.6% efficient)
Worker load:     [665, 754]
Load imbalance:  1.06x
Worker idle:     [1330.3ms, 0.0ms]
===================================================================

===== DAG: Attention Layer (128-head, 1024x1024) =====
Workers:         4
Total time:      2657.6ms
Tasks executed:  1419
Steals:          48 of 41359 attempts (3.4% of tasks, 0.1% efficient)
Worker load:     [364, 351, 288, 416]
Load imbalance:  1.17x
Worker idle:     [1447.3ms, 1435.3ms, 1434.2ms, 0.1ms]
===================================================================

===== DAG: Attention Layer (128-head, 1024x1024) =====
Workers:         8
Total time:      2459.2ms
Tasks executed:  1419
Steals:          56 of 107128 attempts (3.9% of tasks, 0.1% efficient)
Worker load:     [171, 277, 170, 146, 164, 183, 149, 159]
Load imbalance:  1.56x
Worker idle:     [1477.3ms, 12.6ms, 1502.9ms, 1474.4ms, 1493.6ms, 1487.3ms, 1487.6ms, 1483.4ms]
===================================================================
--- Results (baseline = 1 worker) ---

Workers     Time (ms)   Speedup  Steal Rate  Load Distribution
------------------------------------------------------------------------------
1              4490.8     1.00x        0.0%  [1419]
2              4857.5     0.92x        3.8%  [665, 754]
4              2657.6     1.69x        3.4%  [364, 351, 288, 416]
8              2459.2     1.83x        3.9%  [171, 277, 170, 146, 164, 183, 149, 159]

Scheduler overhead at 1 worker: 0.94x sequential

All benchmarks complete.
