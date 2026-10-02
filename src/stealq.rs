//! Part A — the work-stealing queue.

use std::collections::VecDeque;
use std::sync::Mutex;

/// The result of a steal attempt.
///
/// Note the three-way distinction: a thief that finds nothing (`Vacant`)
/// should move on to a different victim, but a thief that loses a race
/// (`Contended`) knows there *was* work there and may want to retry.
#[derive(Debug, PartialEq)]
pub enum StealOutcome<T> {
    /// Successfully took an item from the victim.
    Took(T),
    /// The victim's queue was empty.
    Vacant,
    /// Another thief won the race (retry, or try someone else).
    Contended,
}

/// A work-stealing queue.
///
/// - The **owner** thread pushes and pops from the **bottom** (LIFO).
/// - **Thieves** (other threads) steal from the **top** (FIFO).
///
/// This decoupling means the owner gets cache-local tasks (recently
/// pushed = recently touched data), while thieves get older tasks that
/// are more likely to unlock a large subtree of work — which amortises
/// the cost of the steal itself.
///
/// # Implementation Choice
///
/// You may implement this as:
///   - **Two-Lock Queue**: simpler, uses a Mutex (or two) to guard the ends.
///   - **Chase–Lev Queue**: lock-free owner path, CAS-based steal. (Bonus)
///
/// Document your choice and your reasoning in the writeup.
pub struct StealQueue<T> {
    // TODO: Choose your internal representation.
    //
    // Option 1 (Single/Two-Lock):
    //   inner: Mutex<VecDeque<T>>,
    //
    //   Simplest version: one lock, all operations contend on it.
    //   A better variant uses two Mutexes (one for the owner end, one
    //   for the thief end) but requires care when the queue holds 0 or
    //   1 elements — both ends refer to the same slot.
    //
    // Option 2 (Chase–Lev):
    //   top: AtomicIsize,
    //   bottom: AtomicIsize,
    //   buffer: AtomicPtr<CircularBuffer<T>>,
    //
    //   Lock-free on the owner path. More complex; see the bonus section.
    //
    // The single-lock skeleton is provided as a starting point.
    // You are free to replace it entirely.
    inner: Mutex<VecDeque<T>>,
}

impl<T: Send> StealQueue<T> {
    /// Create a new empty queue with room for at least `capacity` items.
    ///
    /// Pre-allocating matters here: a queue that reallocates while a
    /// thief is mid-steal is exactly the hard case in a lock-free design,
    /// and even in the locked design it keeps the owner's push path cheap.
    pub fn with_capacity(capacity: usize) -> Self {
        // TODO: Implement with_capacity.
        //
        // For the locked version:
        //   Mutex::new(VecDeque::with_capacity(capacity))
        let _ = capacity;
        todo!("Implement StealQueue::with_capacity")
    }

    /// Push an item to the bottom of the queue.
    ///
    /// Called only by the **owner** thread.
    pub fn push(&self, item: T) {
        // TODO: Implement push.
        //
        // Locked version:
        //   Lock the mutex, push_back(item).
        //
        // Chase–Lev:
        //   Write item at buffer[bottom], then increment bottom (Release).
        //   Grow the buffer first if bottom - top >= capacity.
        let _ = item;
        todo!("Implement StealQueue::push")
    }

    /// Pop an item from the bottom of the queue (LIFO).
    ///
    /// Called only by the **owner** thread.
    /// Returns `None` if the queue is empty.
    pub fn pop(&self) -> Option<T> {
        // TODO: Implement pop.
        //
        // Locked version:
        //   Lock the mutex, pop_back().
        //
        // Chase–Lev:
        //   Decrement bottom. If bottom > top, return the item.
        //   If bottom == top, CAS top to top+1 — you are racing a thief
        //   for the last element. On success return the item.
        //   Either way, reset bottom = top + 1 before returning.
        todo!("Implement StealQueue::pop")
    }

    /// Steal an item from the top of the queue (FIFO).
    ///
    /// Called by **thief** threads (any thread other than the owner).
    /// Returns `StealOutcome::Took(item)` if an item was taken,
    /// `StealOutcome::Vacant` if the queue is empty, or
    /// `StealOutcome::Contended` if another thief won the race.
    pub fn steal(&self) -> StealOutcome<T> {
        // TODO: Implement steal.
        //
        // Locked version:
        //   Use try_lock (NOT lock).
        //   - Lock acquired: pop_front(). Return Took or Vacant.
        //   - Lock not acquired: return Contended.
        //   Using try_lock is what makes stealing non-blocking: a thief
        //   must never block the owner's fast path.
        //
        // Chase–Lev:
        //   Read top (Acquire), then bottom (Acquire).
        //   If top >= bottom, return Vacant.
        //   Read the item at buffer[top].
        //   CAS top from old to old+1.
        //   On success return Took(item); otherwise return Contended.
        todo!("Implement StealQueue::steal")
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_push_pop_lifo() {
        let q = StealQueue::with_capacity(8);
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.pop(), Some(3)); // LIFO for the owner
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn test_steal_fifo() {
        let q = StealQueue::with_capacity(8);
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.steal(), StealOutcome::Took(1)); // FIFO for thieves
        assert_eq!(q.steal(), StealOutcome::Took(2));
        assert_eq!(q.steal(), StealOutcome::Took(3));
        assert_eq!(q.steal(), StealOutcome::Vacant);
    }

    #[test]
    fn test_push_pop_interleaved() {
        let q = StealQueue::with_capacity(8);
        q.push(1);
        q.push(2);
        assert_eq!(q.pop(), Some(2));
        q.push(3);
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), Some(1));
    }

    #[test]
    fn test_steal_empty() {
        let q = StealQueue::<i32>::with_capacity(4);
        assert_eq!(q.steal(), StealOutcome::Vacant);
    }

    #[test]
    fn test_opposite_ends() {
        // With two items, the owner and a thief must get different ones.
        let q = StealQueue::with_capacity(4);
        q.push(10);
        q.push(20);
        assert_eq!(q.pop(), Some(20)); // owner takes the bottom
        assert_eq!(q.steal(), StealOutcome::Took(10)); // thief takes the top
        assert_eq!(q.steal(), StealOutcome::Vacant);
    }

    #[test]
    fn test_concurrent_steal() {
        let q = Arc::new(StealQueue::with_capacity(1024));
        let n = 1000;

        // Owner pushes N items.
        for i in 0..n {
            q.push(i);
        }

        // 4 thieves steal concurrently.
        let mut handles = vec![];
        for _ in 0..4 {
            let q = Arc::clone(&q);
            handles.push(thread::spawn(move || {
                let mut stolen = vec![];
                loop {
                    match q.steal() {
                        StealOutcome::Took(v) => stolen.push(v),
                        StealOutcome::Vacant => break,
                        StealOutcome::Contended => continue,
                    }
                }
                stolen
            }));
        }

        let mut all: Vec<i32> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        all.sort();

        // No item lost, no item duplicated.
        assert_eq!(all.len(), n as usize);
        for (i, v) in all.iter().enumerate() {
            assert_eq!(*v, i as i32);
        }
    }

    #[test]
    fn test_owner_and_thieves() {
        let q = Arc::new(StealQueue::with_capacity(1024));
        let n = 500;

        for i in 0..n {
            q.push(i);
        }

        // Owner pops from the bottom while a thief steals from the top.
        let q2 = Arc::clone(&q);
        let thief = thread::spawn(move || {
            let mut stolen = vec![];
            loop {
                match q2.steal() {
                    StealOutcome::Took(v) => stolen.push(v),
                    StealOutcome::Vacant => break,
                    StealOutcome::Contended => {}
                }
            }
            stolen
        });

        let mut popped = vec![];
        while let Some(v) = q.pop() {
            popped.push(v);
        }

        let stolen = thief.join().unwrap();
        let mut all: Vec<i32> = popped.into_iter().chain(stolen).collect();
        all.sort();

        // Every item should appear exactly once.
        assert_eq!(all.len(), n as usize);
        for (i, v) in all.iter().enumerate() {
            assert_eq!(*v, i as i32);
        }
    }

    #[test]
    fn test_stress_many_thieves() {
        // Higher contention: 8 thieves against a live owner.
        let q = Arc::new(StealQueue::with_capacity(4096));
        let n = 4000;
        for i in 0..n {
            q.push(i);
        }

        let mut handles = vec![];
        for _ in 0..8 {
            let q = Arc::clone(&q);
            handles.push(thread::spawn(move || {
                let mut stolen = vec![];
                let mut empty_rounds = 0;
                loop {
                    match q.steal() {
                        StealOutcome::Took(v) => {
                            stolen.push(v);
                            empty_rounds = 0;
                        }
                        StealOutcome::Vacant => {
                            empty_rounds += 1;
                            if empty_rounds > 64 {
                                break;
                            }
                        }
                        StealOutcome::Contended => {}
                    }
                }
                stolen
            }));
        }

        let mut popped = vec![];
        while let Some(v) = q.pop() {
            popped.push(v);
        }

        let mut all: Vec<i32> = popped;
        for h in handles {
            all.extend(h.join().unwrap());
        }
        all.sort();

        assert_eq!(all.len(), n as usize, "items lost or duplicated");
        for (i, v) in all.iter().enumerate() {
            assert_eq!(*v, i as i32);
        }
    }
}
