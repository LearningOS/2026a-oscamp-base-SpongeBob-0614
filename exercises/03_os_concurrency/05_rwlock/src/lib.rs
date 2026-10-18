//! # Read-Write Lock (Writer-Priority)
//!
//! In this exercise, you will implement a **writer-priority** read-write lock from scratch using atomics.
//! Multiple readers may hold the lock concurrently; a writer holds it exclusively.
//!
//! **Note:** Rust's standard library already provides [`std::sync::RwLock`]. This exercise implements
//! a minimal version for learning the protocol and policy without using the standard one.
//!
//! ## Common policies for read-write locks
//! Different implementations can give different **priority** when both readers and writers are waiting:
//!
//! - **Reader-priority (读者优先)**: New readers are allowed to enter while a writer is waiting, so writers
//!   may be starved if readers keep arriving.
//! - **Writer-priority (写者优先)**: Once a writer is waiting, no new readers are admitted until that writer
//!   has run; this exercise implements this policy.
//! - **Read-write fair (读写公平)**: Requests are served in a fair order (e.g. FIFO or round-robin), so
//!   neither readers nor writers are systematically starved.
//!
//! ## Key Concepts
//! - **Readers**: share access; many threads can hold a read lock at once.
//! - **Writer**: exclusive access; only one writer, and no readers while the writer holds the lock.
//! - **Writer-priority (this implementation)**: when at least one writer is waiting, new readers block
//!   until the writer runs.
//!
//! ## State (single atomic)
//! We use one `AtomicU64`: low bits = reader count, a writer-holding flag, and a waiting-writer count.
//! All logic is implemented with compare_exchange and load/store; no use of `std::sync::RwLock`.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum number of concurrent readers (fits in state bits).
const READER_MASK: u64 = (1 << 30) - 1;
/// Bit set when a writer holds the lock.
const WRITER_HOLDING: u64 = 1 << 30;
/// Each waiting writer contributes one unit in the high bits. A count keeps
/// readers blocked when one writer acquires the lock but others still wait.
const WRITER_WAITING: u64 = 1 << 31;
const WRITER_WAITING_MASK: u64 = !(WRITER_WAITING - 1);

/// Writer-priority read-write lock. Implemented from scratch; does not use `std::sync::RwLock`.
pub struct RwLock<T> {
    state: AtomicU64,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU64::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire a read lock. Blocks (spins) until no writer holds and no writer is waiting (writer-priority).
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & (WRITER_HOLDING | WRITER_WAITING_MASK) != 0
                || state & READER_MASK == READER_MASK
            {
                core::hint::spin_loop();
                continue;
            }
            if self
                .state
                .compare_exchange_weak(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return RwLockReadGuard { lock: self };
            }
        }
    }

    /// Acquire the write lock. Blocks until no readers and no other writer.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        // Register this writer before waiting, blocking new readers. Use CAS
        // rather than fetch_add so a saturated count can never wrap to zero.
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if let Some(waiting) = state.checked_add(WRITER_WAITING) {
                if self
                    .state
                    .compare_exchange_weak(state, waiting, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
            core::hint::spin_loop();
        }
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & (READER_MASK | WRITER_HOLDING) == 0
                && self
                    .state
                    .compare_exchange_weak(
                        state,
                        (state - WRITER_WAITING) | WRITER_HOLDING,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                return RwLockWriteGuard { lock: self };
            }
            core::hint::spin_loop();
        }
    }
}

/// Guard for a read lock; releases the read lock on drop.
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: a read guard excludes writers for its entire lifetime.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

/// Guard for a write lock; releases the write lock on drop.
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the write guard has exclusive access until it drops.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the write guard is exclusive, and &mut self prevents aliases.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        // Preserve other waiting writers so readers cannot jump the queue.
        self.lock
            .state
            .fetch_and(!WRITER_HOLDING, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_multiple_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(*g, 0);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_writer_excludes_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let lock_w = Arc::clone(&lock);
        let writer = thread::spawn(move || {
            let mut g = lock_w.write();
            *g = 42;
        });
        writer.join().unwrap();
        let g = lock.read();
        assert_eq!(*g, 42);
    }

    #[test]
    fn test_concurrent_reads_after_write() {
        let lock = Arc::new(RwLock::new(Vec::<i32>::new()));
        {
            let mut g = lock.write();
            g.push(1);
            g.push(2);
        }
        let mut handles = vec![];
        for _ in 0..5 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(g.len(), 2);
                assert_eq!(&*g, &[1, 2]);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_writes_serialized() {
        let lock = Arc::new(RwLock::new(0u64));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut g = l.write();
                    *g += 1;
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*lock.read(), 1000);
    }

    #[test]
    fn test_all_waiting_writers_precede_new_readers() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let lock = Arc::new(RwLock::new(0));
        let initial_reader = lock.read();
        let (events, received) = mpsc::channel();
        let mut releases = Vec::new();
        let mut writers = Vec::new();
        for id in 0..2 {
            let lock = Arc::clone(&lock);
            let events = events.clone();
            let (release, released) = mpsc::channel();
            releases.push(release);
            writers.push(thread::spawn(move || {
                let mut guard = lock.write();
                *guard += 1;
                events.send(id).unwrap();
                released.recv_timeout(Duration::from_secs(5)).unwrap();
            }));
        }
        // Observe registration before introducing the new reader. Both writers
        // must remain counted even when the first one starts holding the lock.
        let deadline = Instant::now() + Duration::from_secs(5);
        while lock.state.load(Ordering::Relaxed) & WRITER_WAITING_MASK < 2 * WRITER_WAITING {
            assert!(Instant::now() < deadline, "writers did not register");
            thread::yield_now();
        }
        let reader_lock = Arc::clone(&lock);
        let reader = thread::spawn(move || {
            let guard = reader_lock.read();
            events.send(2).unwrap();
            assert_eq!(*guard, 2);
        });
        drop(initial_reader);
        for _ in 0..2 {
            let id = received.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(id < 2, "a reader bypassed a waiting writer");
            releases[id].send(()).unwrap();
        }
        assert_eq!(received.recv_timeout(Duration::from_secs(5)).unwrap(), 2);
        for writer in writers {
            writer.join().unwrap();
        }
        reader.join().unwrap();
    }
}
