//! # RAII Spin Lock Guard
//!
//! In this exercise, you will implement an RAII guard for a spin lock, causing the lock to be automatically released when leaving scope.
//! This is a classic application of Rust's ownership system in systems programming.
//!
//! ## Key Points
//! - RAII (Resource Acquisition Is Initialization) pattern
//! - `Deref` / `DerefMut` traits for transparent access
//! - `Drop` trait for automatic release
//! - Why manual lock/unlock is unsafe (forgetting unlock, panic without release)

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}
unsafe impl<T: Send> Send for SpinLock<T> {}

/// Spin lock guard: RAII handle holding the lock.
/// Automatically releases the lock when SpinGuard is dropped.
/// A guard cannot be shared between threads when its data is not `Sync`:
///
/// ```compile_fail
/// use spinlock_guard::SpinLock;
/// use std::cell::Cell;
/// fn require_sync<T: Sync>(_: &T) {}
/// let lock = SpinLock::new(Cell::new(0));
/// require_sync(&lock.lock());
/// ```
pub struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
    // Sharing a guard also shares &T, so Sync requires T: Sync.
    marker: PhantomData<&'a mut T>,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire lock, returning SpinGuard.
    pub fn lock(&self) -> SpinGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinGuard {
            lock: self,
            marker: PhantomData,
        }
    }
}

impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard holds the lock until it is dropped.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard holds the lock and &mut self prevents aliases.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_guard_auto_release() {
        let lock = SpinLock::new(0u32);
        {
            let mut guard = lock.lock();
            *guard = 42;
            // guard drops here, automatically releasing lock
        }
        // Should be able to acquire lock again
        let guard = lock.lock();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_guard_deref() {
        let lock = SpinLock::new(String::from("hello"));
        let guard = lock.lock();
        assert_eq!(guard.len(), 5);
        assert_eq!(&*guard, "hello");
    }

    #[test]
    fn test_guard_deref_mut() {
        let lock = SpinLock::new(Vec::<i32>::new());
        {
            let mut guard = lock.lock();
            guard.push(1);
            guard.push(2);
            guard.push(3);
        }
        let guard = lock.lock();
        assert_eq!(&*guard, &[1, 2, 3]);
    }

    #[test]
    fn test_concurrent_with_guard() {
        let lock = Arc::new(SpinLock::new(0u64));
        let mut handles = vec![];

        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = l.lock();
                    *guard += 1;
                    // guard automatically released
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(*lock.lock(), 10000);
    }

    #[test]
    fn test_panic_safety() {
        let lock = Arc::new(SpinLock::new(0u32));
        let l = Arc::clone(&lock);

        let result = thread::spawn(move || {
            let mut guard = l.lock();
            *guard = 42;
            panic!("intentional panic");
        })
        .join();

        assert!(result.is_err());
        // Even if thread panics, guard's Drop should release lock
        // Note: this test may have different results due to panic unwind behavior
    }

    #[test]
    fn test_guard_releases_lock_during_unwinding() {
        let lock = Arc::new(SpinLock::new(0));
        let other = Arc::clone(&lock);
        assert!(thread::spawn(move || {
            *other.lock() = 42;
            let _guard = other.lock();
            panic!("unwind while holding the lock");
        })
        .join()
        .is_err());
        assert_eq!(*lock.lock(), 42);
    }
}
