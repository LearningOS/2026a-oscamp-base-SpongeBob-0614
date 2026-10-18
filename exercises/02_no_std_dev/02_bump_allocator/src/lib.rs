//! # Bump Allocator (no_std)
//!
//! Implement the simplest heap memory allocator: a Bump Allocator (bump pointer allocator).
//!
//! ## How It Works
//!
//! A Bump Allocator maintains a pointer `next` to the "next available address".
//! On each allocation, it aligns `next` to the requested alignment, then advances by `size` bytes.
//! It does not support freeing individual objects (`dealloc` is a no-op).
//!
//! ```text
//! heap_start                              heap_end
//! |----[allocated]----[allocated]----| next |---[free]---|
//!                                        ^
//!                                    next allocation starts here
//! ```
//!
//! ## Task
//!
//! Implement `BumpAllocator`'s `GlobalAlloc::alloc` method:
//! 1. Align the current `next` up to `layout.align()`
//!    Hint: `align_up(addr, align) = (addr + align - 1) & !(align - 1)`
//! 2. Check if the aligned address plus `layout.size()` exceeds `heap_end`
//! 3. If it exceeds, return `null_mut()`; otherwise atomically update `next` with `compare_exchange`
//!
//! ## Key Concepts
//!
//! - `core::alloc::{GlobalAlloc, Layout}`
//! - Memory alignment calculation
//! - `AtomicUsize` and `compare_exchange` (CAS loop)

#![cfg_attr(not(test), no_std)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    /// Create a new BumpAllocator.
    ///
    /// # Safety
    /// `heap_start..heap_end` must be a valid, readable and writable memory region,
    /// and must not be used by other code during this allocator's lifetime.
    pub const unsafe fn new(heap_start: usize, heap_end: usize) -> Self {
        Self {
            heap_start,
            heap_end,
            next: AtomicUsize::new(heap_start),
        }
    }

    /// Reset the allocator (free all allocated memory).
    ///
    /// Before resetting, the caller must stop using all previous allocations
    /// and ensure no other thread is allocating or accessing this heap.
    pub fn reset(&self) {
        self.next.store(self.heap_start, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut next = self.next.load(Ordering::SeqCst);
        loop {
            let Some(aligned) = next
                .checked_add(layout.align() - 1)
                .map(|addr| addr & !(layout.align() - 1))
            else {
                return null_mut();
            };
            let Some(end) = aligned.checked_add(layout.size()) else {
                return null_mut();
            };
            if end > self.heap_end {
                return null_mut();
            }
            match self
                .next
                .compare_exchange(next, end, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return aligned as *mut u8,
                Err(current) => next = current,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator does not reclaim individual objects — leave empty
    }
}

// ============================================================
// Tests
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    const HEAP_SIZE: usize = 4096;

    fn make_allocator() -> (BumpAllocator, Vec<u8>) {
        let mut heap = vec![0u8; HEAP_SIZE];
        let start = heap.as_mut_ptr() as usize;
        let alloc = unsafe { BumpAllocator::new(start, start + HEAP_SIZE) };
        (alloc, heap)
    }

    #[test]
    fn test_alloc_basic() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(16, 8).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null(), "allocation should succeed");
    }

    #[test]
    fn test_alloc_alignment() {
        let (alloc, _heap) = make_allocator();
        for align in [1, 2, 4, 8, 16, 64] {
            let layout = Layout::from_size_align(1, align).unwrap();
            let ptr = unsafe { alloc.alloc(layout) };
            assert!(!ptr.is_null());
            assert_eq!(
                ptr as usize % align,
                0,
                "returned address must satisfy align={align}"
            );
        }
    }

    #[test]
    fn test_alloc_no_overlap() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(64, 8).unwrap();
        let p1 = unsafe { alloc.alloc(layout) } as usize;
        let p2 = unsafe { alloc.alloc(layout) } as usize;
        assert!(
            p1 + 64 <= p2 || p2 + 64 <= p1,
            "two allocations must not overlap"
        );
    }

    #[test]
    fn test_alloc_oom() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(HEAP_SIZE + 1, 1).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(ptr.is_null(), "should return null when exceeding heap");
    }

    #[test]
    fn test_alloc_fill_heap() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(256, 1).unwrap();
        for i in 0..16 {
            let ptr = unsafe { alloc.alloc(layout) };
            assert!(!ptr.is_null(), "allocation #{i} should succeed");
        }
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(ptr.is_null(), "should return null when heap is full");
    }

    #[test]
    fn test_address_overflow_returns_null() {
        // These allocators only exercise arithmetic; no returned address is dereferenced.
        let alloc = BumpAllocator {
            heap_start: usize::MAX - 7,
            heap_end: usize::MAX,
            next: AtomicUsize::new(usize::MAX - 7),
        };
        assert!(unsafe { alloc.alloc(Layout::from_size_align(1, 16).unwrap()) }.is_null());
        assert!(unsafe { alloc.alloc(Layout::from_size_align(8, 1).unwrap()) }.is_null());
    }

    #[test]
    fn test_concurrent_allocations_do_not_overlap() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(16, 8).unwrap();
        let mut addresses = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let alloc = &alloc;
                    scope.spawn(move || {
                        (0..32)
                            .map(|_| unsafe { alloc.alloc(layout) } as usize)
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        addresses.sort_unstable();
        assert_ne!(addresses[0], 0);
        assert!(addresses.windows(2).all(|pair| pair[0] + 16 <= pair[1]));
    }

    #[test]
    fn test_reset() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(HEAP_SIZE, 1).unwrap();
        let p1 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null());
        alloc.reset();
        let p2 = unsafe { alloc.alloc(layout) };
        assert!(!p2.is_null(), "should be able to allocate after reset");
        assert_eq!(
            p1, p2,
            "address after reset should match the first allocation"
        );
    }
}
