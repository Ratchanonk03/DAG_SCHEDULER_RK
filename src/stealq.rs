//! Part A — the work-stealing queue.
use std::ptr;
use std::sync::atomic::{fence, AtomicUsize, AtomicPtr, Ordering};
use std::sync::{Mutex};

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

struct Buffer<T> {
    capacity: usize,
    slots: Box<[AtomicPtr<T>]>,
}

impl<T> Buffer<T> {
    fn new(capacity: usize) -> Self {
        let capacity = capacity
            .max(2)
            .checked_next_power_of_two()
            .expect("StealQueue capacity is too large");

        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, || AtomicPtr::new(ptr::null_mut()));

        Self {
            capacity,
            slots: slots.into_boxed_slice(),
        }
    }

    fn grow(&self, top: usize, bottom: usize) -> Box<Buffer<T>> {
        let new = Box::new(Buffer::new(self.capacity * 2));

        for index in top..bottom {
            let item = self.slot(index).load(Ordering::Acquire);
            new.slot(index).store(item, Ordering::Relaxed);
        }

        new
    }
    
    #[inline]
    fn slot(&self, index: usize) -> &AtomicPtr<T> {
        &self.slots[index & (self.capacity - 1)]
    }


    #[inline]
    unsafe fn take_slot(&self, index: usize) -> Option<T> {
        // Safety: the caller must have already claimed this queue position, so no
        // other thread can convert the same raw pointer back into a `Box<T>`.
       let item = self.slot(index).swap(ptr::null_mut(), Ordering::AcqRel);
        (!item.is_null()).then(|| *Box::from_raw(item))
    }
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
    // TODO(DONE): Choose your internal representation.
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
    top: AtomicUsize,
    bottom: AtomicUsize,
    buffer: AtomicPtr<Buffer<T>>,
    retired: Mutex<Vec<*mut Buffer<T>>>,
}

unsafe impl<T: Send> Send for StealQueue<T> {}
unsafe impl<T: Send> Sync for StealQueue<T> {}

impl<T: Send> StealQueue<T> {
    /// Create a new empty queue with room for at least `capacity` items.
    ///
    /// Pre-allocating matters here: a queue that reallocates while a
    /// thief is mid-steal is exactly the hard case in a lock-free design,
    /// and even in the locked design it keeps the owner's push path cheap.
    pub fn with_capacity(capacity: usize) -> Self {
        // TODO(DONE): Implement with_capacity.
        //
        // For the locked version:
        //   Mutex::new(VecDeque::with_capacity(capacity))
        let buffer = Box::into_raw(Box::new(Buffer::new(capacity)));

        Self {
            top: AtomicUsize::new(0),
            bottom: AtomicUsize::new(0),
            buffer: AtomicPtr::new(buffer),
            retired: Mutex::new(Vec::new()),
        }
    }

    /// Push an item to the bottom of the queue.
    ///
    /// Called only by the **owner** thread.
    pub fn push(&self, item: T) {
        // TODO(DONE): Implement push.
        //
        // Locked version:
        //   Lock the mutex, push_back(item).
        //
        // Chase–Lev:
        //   Write item at buffer[bottom], then increment bottom (Release).
        //   Grow the buffer first if bottom - top >= capacity.
        let bottom = self.bottom.load(Ordering::Relaxed);
        let top = self.top.load(Ordering::Acquire);
        let mut buffer = self.buffer.load(Ordering::Acquire);

        // Safety: `buffer` is initialized in `with_capacity`, and buffers are
        // retained until Drop, so it remains valid for the queue's lifetime.
        if bottom.wrapping_sub(top) >= unsafe { (*buffer).capacity - 1 } {
            buffer = self.grow(buffer, top, bottom);
        }

        // Safety: only the owner writes at `bottom`; the slot is published by
        // the Release store to `bottom` below.
        unsafe {
            (*buffer)
                .slot(bottom)
                .store(Box::into_raw(Box::new(item)), Ordering::Release);
        }

        self.bottom.store(bottom.wrapping_add(1), Ordering::Release);
    }

    /// Pop an item from the bottom of the queue (LIFO).
    ///
    /// Called only by the **owner** thread.
    /// Returns `None` if the queue is empty.
    pub fn pop(&self) -> Option<T> {
        // TODO(DONE): Implement pop.
        //
        // Locked version:
        //   Lock the mutex, pop_back().
        //
        // Chase–Lev:
        //   Decrement bottom. If bottom > top, return the item.
        //   If bottom == top, CAS top to top+1 — you are racing a thief
        //   for the last element. On success return the item.
        //   Either way, reset bottom = top + 1 before returning.
       let bottom = self.bottom.load(Ordering::Relaxed);
        if bottom == 0 {
            return None;
        }

        let bottom = bottom.wrapping_sub(1);
        self.bottom.store(bottom, Ordering::Relaxed);
        fence(Ordering::SeqCst);

        let top = self.top.load(Ordering::Acquire);

        if top > bottom {
            self.bottom.store(top, Ordering::Relaxed);
            return None;
        }

        let buffer = self.buffer.load(Ordering::Acquire);

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

        // Safety: the owner is either is the only thread that accesses a bottom slot
        // or won the CAS race for the last item. Therefore no other thread can take this slot.
        unsafe { (*buffer).take_slot(bottom) }
    }

    /// Steal an item from the top of the queue (FIFO).
    ///
    /// Called by **thief** threads (any thread other than the owner).
    /// Returns `StealOutcome::Took(item)` if an item was taken,
    /// `StealOutcome::Vacant` if the queue is empty, or
    /// `StealOutcome::Contended` if another thief won the race.
    pub fn steal(&self) -> StealOutcome<T> {
        // TODO(DONE): Implement steal.
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

        let top = self.top.load(Ordering::Acquire);
        fence(Ordering::SeqCst);

        let bottom = self.bottom.load(Ordering::Acquire);
        if top >= bottom {
            return StealOutcome::Vacant;
        }

        let buffer = self.buffer.load(Ordering::Acquire);

        match self.top.compare_exchange(
            top,
            top.wrapping_add(1),
            Ordering::SeqCst,
            Ordering::Relaxed,
        ){
            Ok(_) => {
                // Safety: winning the CAS gives this thief exclusive ownership of `top`.
                let item = unsafe { (*buffer).take_slot(top) };
                StealOutcome::Took(item.expect("slot should be non-null"))
            },
            Err(_) => {
                return StealOutcome::Contended;
            }
        }

    }

    fn grow(&self, old: *mut Buffer<T>, top: usize, bottom: usize) -> *mut Buffer<T> {
        let new = unsafe { Box::into_raw((&*old).grow(top, bottom)) };

        self.buffer.store(new, Ordering::Release);
        self.retired
            .lock()
            .expect("retired-buffer mutex poisoned")
            .push(old);

        new
    }
        
}

impl<T> Drop for StealQueue<T> {
    fn drop(&mut self) {
        let top = self.top.load(Ordering::Relaxed);
        let bottom = self.bottom.load(Ordering::Relaxed);
        let current = self.buffer.load(Ordering::Relaxed);

        // Safety: Drop has exclusive access to the queue. Only the current
        // buffer owns live item pointers; retired buffers contain stale copies.
        unsafe {
            for index in top..bottom {
                drop((*current).take_slot(index));
            }

            drop(Box::from_raw(current));

            for old in self
                .retired
                .get_mut()
                .expect("retired-buffer mutex poisoned")
                .drain(..)
            {
                drop(Box::from_raw(old));
            }
        }
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
