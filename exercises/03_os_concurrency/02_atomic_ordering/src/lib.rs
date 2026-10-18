//! # Memory Ordering and Synchronization
//!
//! In this exercise, you will use correct memory ordering to implement thread synchronization primitives.
//!
//! ## Key Concepts
//! - `Ordering::Relaxed`: No synchronization guarantees
//! - `Ordering::Acquire`: Read operation, prevents subsequent reads/writes from being reordered before this operation
//! - `Ordering::Release`: Write operation, prevents preceding reads/writes from being reordered after this operation
//! - `Ordering::AcqRel`: Both Acquire and Release semantics
//! - `Ordering::SeqCst`: Sequentially consistent (global ordering)
//!
//! ## Release-Acquire Pairing
//! When thread A writes with Release, and thread B reads the same location with Acquire,
//! thread B will see all writes that thread A performed before the Release.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

/// Use Release-Acquire semantics to safely pass data between two threads.
///
/// `produce` writes data first, then sets flag with Release;
/// `consume` reads flag with Acquire, ensuring it sees the data.
pub struct FlagChannel {
    data: AtomicU32,
    ready: AtomicBool,
}

impl FlagChannel {
    pub const fn new() -> Self {
        Self {
            data: AtomicU32::new(0),
            ready: AtomicBool::new(false),
        }
    }

    /// Producer: store data first, then set ready flag.
    /// Release publishes the preceding data store to an acquiring consumer.
    pub fn produce(&self, value: u32) {
        self.data.store(value, Ordering::Relaxed);
        self.ready.store(true, Ordering::Release);
    }

    /// Consumer: spin-wait for ready flag, then read data.
    /// Acquire pairs with the producer's Release before reading the data.
    pub fn consume(&self) -> u32 {
        while !self.ready.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
        self.data.load(Ordering::Relaxed)
    }

    /// Reset channel state after the producer and consumer have finished.
    pub fn reset(&self) {
        self.ready.store(false, Ordering::Relaxed);
        self.data.store(0, Ordering::Relaxed);
    }
}

/// A simple once-initializer using SeqCst.
/// Guarantees `init` is executed only once, and all threads see the initialized value.
pub struct OnceCell {
    // 0 = empty, 1 = initialization claimed, 2 = value published.
    state: AtomicU8,
    value: AtomicU32,
}

impl OnceCell {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
            value: AtomicU32::new(0),
        }
    }

    /// Attempt initialization. If not yet initialized, store value and return true.
    /// If already initialized, return false.
    ///
    /// Hint: use `compare_exchange` to ensure only one thread succeeds.
    pub fn init(&self, val: u32) -> bool {
        if self
            .state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return false;
        }
        self.value.store(val, Ordering::SeqCst);
        // Claiming initialization is separate from publishing the value: a
        // reader must never observe the default value while init is in progress.
        self.state.store(2, Ordering::SeqCst);
        true
    }

    /// Get value. Returns Some if initialized, otherwise None.
    pub fn get(&self) -> Option<u32> {
        if self.state.load(Ordering::SeqCst) == 2 {
            Some(self.value.load(Ordering::SeqCst))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_flag_channel() {
        let ch = Arc::new(FlagChannel::new());
        let ch2 = Arc::clone(&ch);

        let producer = thread::spawn(move || {
            ch2.produce(42);
        });

        let consumer = thread::spawn(move || ch.consume());

        producer.join().unwrap();
        let val = consumer.join().unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn test_flag_channel_large_value() {
        let ch = Arc::new(FlagChannel::new());
        let ch2 = Arc::clone(&ch);

        let producer = thread::spawn(move || {
            ch2.produce(0xDEAD_BEEF);
        });

        let val = ch.consume();
        producer.join().unwrap();
        assert_eq!(val, 0xDEAD_BEEF);
    }

    #[test]
    fn test_once_cell_init_once() {
        let cell = OnceCell::new();
        assert!(cell.init(42));
        assert!(!cell.init(100));
        assert_eq!(cell.get(), Some(42));
    }

    #[test]
    fn test_once_cell_not_initialized() {
        let cell = OnceCell::new();
        assert_eq!(cell.get(), None);
    }

    #[test]
    fn test_once_cell_concurrent() {
        let cell = Arc::new(OnceCell::new());
        let mut handles = vec![];

        for i in 0..10 {
            let c = Arc::clone(&cell);
            handles.push(thread::spawn(move || c.init(i)));
        }

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        // Exactly one thread initializes successfully
        assert_eq!(results.iter().filter(|&&r| r).count(), 1);
        assert!(cell.get().is_some());
    }

    #[test]
    fn test_once_cell_only_publishes_the_winning_value() {
        for _ in 0..32 {
            let cell = Arc::new(OnceCell::new());
            let barrier = Arc::new(std::sync::Barrier::new(5));
            let handles: Vec<_> = (1..=4)
                .map(|value| {
                    let cell = Arc::clone(&cell);
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        cell.init(value).then_some(value)
                    })
                })
                .collect();
            barrier.wait();
            let published = loop {
                if let Some(value) = cell.get() {
                    break value;
                }
                thread::yield_now();
            };
            let winners: Vec<_> = handles
                .into_iter()
                .filter_map(|handle| handle.join().unwrap())
                .collect();
            assert_eq!(winners, vec![published]);
            assert!((1..=4).contains(&published));
        }
    }

    #[test]
    fn test_once_cell_can_publish_zero() {
        let cell = OnceCell::new();
        assert!(cell.init(0));
        assert_eq!(cell.get(), Some(0));
        assert!(!cell.init(1));
    }
}
