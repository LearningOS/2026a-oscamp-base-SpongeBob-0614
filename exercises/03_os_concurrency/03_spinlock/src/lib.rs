//! # Spin Lock
//!
//! In this exercise, you will implement a basic spin lock.
//! Spin locks are one of the most fundamental synchronization primitives in OS kernels.
//!
//! ## Key Concepts
//! - Spin locks use busy-waiting to acquire the lock
//! - `AtomicBool`'s `compare_exchange` to implement lock acquisition
//! - `core::hint::spin_loop` to reduce CPU power consumption
//! - `UnsafeCell` provides interior mutability

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Basic spin lock
pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}
unsafe impl<T: Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire lock, returning a mutable reference to inner data.
    ///
    /// Spins until an Acquire compare-exchange succeeds.
    /// Call `unlock` after the returned reference is no longer in use.
    pub fn lock(&self) -> &mut T {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        // SAFETY: the acquired lock grants exclusive access until unlock.
        unsafe { &mut *self.data.get() }
    }

    /// Release lock.
    ///
    /// This low-level teaching API relies on the caller owning the lock and
    /// never using a returned reference after unlocking. The RAII guard in the
    /// next exercise enforces these rules through Rust's type system.
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    /// Try to acquire lock without spinning.
    /// Returns Some(&mut T) on success, None if lock is busy.
    pub fn try_lock(&self) -> Option<&mut T> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            // SAFETY: a successful acquisition grants exclusive access.
            .map(|_| unsafe { &mut *self.data.get() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_lock_unlock() {
        let lock = SpinLock::new(0u32);
        {
            let data = lock.lock();
            *data = 42;
            lock.unlock();
        }
        let data = lock.lock();
        assert_eq!(*data, 42);
        lock.unlock();
    }

    #[test]
    fn test_try_lock() {
        let lock = SpinLock::new(0u32);
        assert!(lock.try_lock().is_some());
        lock.unlock();
    }

    #[test]
    fn test_concurrent_counter() {
        let lock = Arc::new(SpinLock::new(0u64));
        let mut handles = vec![];

        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let data = l.lock();
                    *data += 1;
                    l.unlock();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let data = lock.lock();
        assert_eq!(*data, 10000);
        lock.unlock();
    }

    #[test]
    fn test_lock_protects_data() {
        let lock = Arc::new(SpinLock::new(Vec::new()));
        let mut handles = vec![];

        for i in 0..5 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let data = l.lock();
                data.push(i);
                l.unlock();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let data = lock.lock();
        let mut sorted = data.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4]);
        lock.unlock();
    }

    #[test]
    fn test_try_lock_does_not_release_an_existing_lock() {
        let lock = SpinLock::new(1);
        let value = lock.lock();
        assert!(lock.try_lock().is_none());
        *value = 2;
        lock.unlock();
        assert_eq!(lock.try_lock().map(|value| *value), Some(2));
        lock.unlock();
    }
}
