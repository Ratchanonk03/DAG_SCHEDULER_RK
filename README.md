# DAG_SCHEDULER_RK

## Commands
- **Compiling the project:** cargo build --release
- **Run unit tests:** cargo test
- **Run benchmark:** cargo run --release -- --size `<SIZE OF TENSOR>` --heads `<NUMBER OF ATTENTION HEAD>`

# Write Up
## Deque Implementation Rationale
Chase–Lev was selected for the work-stealing queue over a two-lock deque because its common owner operations use inexpensive atomic reads and writes rather than mutex acquisition and release, reducing synchronization overhead. It also avoids lock-holder blocking in the case of a worker being paused during a steal or local queue operation, which might otherwise prevent other workers from progressing.

## Benchmarks

### Specification

- Model: MacBook Air (Mac14,15")
- Chip: Apple M2
- Physical CPU cores: 8 (4 performance + 4 efficiency)
- Memory: 16 GB

**Note:** Due to different type of core, when scaling from 4 to 8 workers, work may also run on the slower efficiency cores. Therefore, the performance improvement may be smaller than expected and may not scale linearly.

### Diamond (1024x1024, Critical path: 3 nodes)
- Sequential reference: 2.4ms
- Scheduler overhead at 1 worker: 0.49x sequential

| Workers |    Time (ms) |  Speedup | Steal Rate |  Load Distribution       |
|---------|--------------|----------|----------- |--------------------------|
| 1       |         1.2  |   1.00x  |      0.0%  | [4]                      |
| 2       |         1.0  |   1.15x  |     25.0%  | [3, 1]                   |
| 4       |         1.0  |   1.22x  |     25.0%  | [2, 2, 0, 0]             |
| 8       |         1.0  |   1.14x  |     25.0%  | [2, 0, 0, 2, 0, 0, 0, 0] |

### Wide-Fan (width=12, 1024x1024, Critical path: 13 nodes)
- Sequential reference: 11.7ms
- Scheduler overhead at 1 worker: 0.83x sequential

| Workers |    Time (ms) |  Speedup | Steal Rate | Load Distribution         |
|---------|--------------|----------|------------|---------------------------|
| 1       |          9.7 |    1.00x |       0.0% | [24]                      |
| 2       |          6.2 |    1.58x |      16.7% | [9, 15]                   |
| 4       |          5.4 |    1.81x |      37.5% | [4, 4, 4, 12]             |
| 8       |          6.1 |    1.60x |      41.7% | [3, 1, 2, 2, 12, 2, 1, 1] |

### Chain (depth=24, 1024x1024, Critical path: 25 nodes)
- Sequential reference: 30.3ms
- Scheduler overhead at 1 worker: 0.88x sequential

| Workers |    Time (ms) |  Speedup | Steal Rate | Load Distribution         |
|---------|--------------|----------|------------|---------------------------|
| 1       |         26.8 |    1.00x |       0.0% | [25]                      |
| 2       |         27.4 |    0.98x |       0.0% | [25, 0]                   |
| 4       |         27.9 |    0.96x |       0.0% | [25, 0, 0, 0]             |
| 8       |         27.9 |    0.96x |       0.0% | [25, 0, 0, 0, 0, 0, 0, 0] |

### Attention Layer (128-head, 1024x1024, Critical path: 141 nodes)
- Sequential reference: 4799.1ms
- Scheduler overhead at 1 worker: 0.94x sequential

| Workers |    Time (ms) |  Speedup | Steal Rate | Load Distribution                        |
|---------|--------------|----------|------------|------------------------------------------|
| 1       |       4490.8 |    1.00x |       0.0% | [1419]                                   |
| 2       |       4857.5 |    0.92x |       3.8% | [665, 754]                               |
| 4       |       2657.6 |    1.69x |       3.4% | [364, 351, 288, 416]                     |
| 8       |       2459.2 |    1.83x |       3.9% | [171, 277, 170, 146, 164, 183, 149, 159] |

## Sub-linear Speedup Discussion
Adding workers improves performance only while the DAG has independent ready nodes that can execute simultaneously. When a node depends on a predecessor, it cannot execute until that predecessor has finished. If no other nodes are ready, additional workers remain idle. Therefore, adding more workers produces sub-linear speedup rather than speedup proportional to the worker count.

For the 128-head attention transformer DAG, the main constraint is a critical path containing 141 sequential nodes. Even with an unlimited number of workers, these dependent nodes must execute in order, limiting the speedup benefit from additional workers.

Amdahl’s Law can be used to estimate speedup by incorporate both the number of worker and task's constrain:

$$ S = \frac{1}{(1 - P) + \frac{P}{N}} $$

where:
- \(S\) is the theoretical speedup.
- \(P\) is the fraction of the task that can run in parallel.
- \(N\) is the number of processors.
- \(1-P\) is the serial fraction of the program.

Rearranging the equation and substituting the measured scheduler speedups gives an effective serial fraction of about 46% at four workers and 48% at eight workers. These figures are effective rather than purely structural: they include dependency waiting as well as scheduling overhead, synchronization, load imbalance, memory effects, and the M2’s slower efficiency cores.

The corresponding Amdahl ceiling is:

$$S_{\max} = \frac{1}{1-P} \approx 2.1\text{--}2.2\times$$

The measured speedup of 1.83× at eight workers is therefore already close to this practical ceiling. This means that eight workers are near the point where adding more workers provides little additional benefit.

## Chase–Lev Last Item Correctness
The last-item owner–thief contention case is handled in `src/stealq.rs` (lines 201–208):

```rust
// Existing pop() logic.
if top == bottom {
    // Owner and thieves race for the final item.
    if self
        .top
        .compare_exchange(
            top,
            top.wrapping_add(1),
            Ordering::SeqCst,
            Ordering::Relaxed,
        )
        .is_err()
    {
        self.bottom.store(top.wrapping_add(1), Ordering::Relaxed);
        return None;
    }

    self.bottom.store(top.wrapping_add(1), Ordering::Relaxed);
}

unsafe { (*buffer).take_slot(bottom) }
```

When the owner observes `top == bottom` after decrementing `bottom`, exactly one task remains. Both the owner and a thief may try to claim the same logical position. The owner uses CAS to change `top` from `top` to `top + 1`.

- If the CAS succeeds, no thief has claimed the final item. The owner exclusively owns the task, restores `bottom` to `top + 1`, and safely continues to `take_slot(bottom)`.
- If the CAS fails, another thread has already advanced `top` and claimed the task. The owner restores `bottom` to `top + 1` and returns `None`, so it never accesses the task slot.

The successful CAS uses `SeqCst` so that it participates in the same global ordering as the owner and thief SeqCst fences. This guarantees that only one contender wins ownership of the last task. The failure ordering can be `Relaxed` because the owner does not consume task data after a failed CAS; it only restores its owner-only `bottom` index and returns.

Therefore, reaching `take_slot(bottom)` in this branch means the owner has successfully claimed the final logical position, making the unsafe conversion from the raw pointer safe.