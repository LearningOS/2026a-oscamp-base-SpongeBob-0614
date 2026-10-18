//! # Free-List Allocator
//!
//! Building on the bump allocator, implement a Free-List Allocator that supports memory reclamation.
//!
//! ## How It Works
//!
//! A Free-List Allocator uses a linked list to track all freed memory blocks.
//! On allocation, it first searches the list for a suitable block (first-fit strategy);
//! if none is found, it falls back to allocating from the unused region.
//! On deallocation, the block is inserted at the head of the list.
//!
//! ```text
//! free_list -> [block A: 64B] -> [block B: 128B] -> [block C: 32B] -> null
//! ```
//!
//! Each free block stores a `FreeBlock` struct at its head (containing block size and next pointer).
//!
//! ## Task
//!
//! Implement `FreeListAllocator`'s `alloc` and `dealloc` methods:
//!
//! ### alloc
//! 1. Traverse the free_list, find the first block with `size >= layout.size()` and proper alignment (first-fit)
//! 2. If found, remove it from the list and return it
//! 3. If not found, allocate from the `bump` region (same as bump allocator)
//!
//! ### dealloc
//! 1. Write `FreeBlock` header info at the freed block
//! 2. Insert it at the head of free_list
//!
//! ## Key Concepts
//!
//! - Intrusive linked list
//! - `*mut T` read/write: `ptr.write(val)` / `ptr.read()`
//! - Memory alignment checks

#![cfg_attr(not(test), no_std)]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Free block header, stored at the beginning of each free memory block
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

pub struct FreeListAllocator {
    heap_end: usize,
    /// Bump pointer: unallocated region starts here.
    bump_next: AtomicUsize,
    /// Protects the head and every link/header in the intrusive free list.
    free_list_locked: AtomicBool,
    free_list: UnsafeCell<*mut FreeBlock>,
}

// Access to the intrusive list is serialized by free_list_locked.
unsafe impl Send for FreeListAllocator {}
unsafe impl Sync for FreeListAllocator {}

struct FreeListGuard<'a>(&'a FreeListAllocator);

impl Drop for FreeListGuard<'_> {
    fn drop(&mut self) {
        self.0.free_list_locked.store(false, Ordering::Release);
    }
}

impl FreeListAllocator {
    /// # Safety
    /// `heap_start..heap_end` must be a valid readable and writable memory region,
    /// exclusively owned by this allocator for its lifetime.
    pub unsafe fn new(heap_start: usize, heap_end: usize) -> Self {
        Self {
            heap_end,
            bump_next: AtomicUsize::new(heap_start),
            free_list_locked: AtomicBool::new(false),
            free_list: UnsafeCell::new(null_mut()),
        }
    }

    fn lock_free_list(&self) -> FreeListGuard<'_> {
        while self
            .free_list_locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.free_list_locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        FreeListGuard(self)
    }
}

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Ensure block is at least large enough to hold a FreeBlock header (for future dealloc)
        let size = layout.size().max(core::mem::size_of::<FreeBlock>());
        let align = layout.align().max(core::mem::align_of::<FreeBlock>());

        {
            let _guard = self.lock_free_list();
            // `link` points to the head or a previous block's next field, so
            // removing either the first or a later block follows the same path.
            let mut link = self.free_list.get();
            while !unsafe { *link }.is_null() {
                let block = unsafe { *link };
                if block as usize % align == 0 && unsafe { (*block).size } >= size {
                    unsafe { *link = (*block).next };
                    return block.cast();
                }
                link = unsafe { core::ptr::addr_of_mut!((*block).next) };
            }
        }

        let mut next = self.bump_next.load(Ordering::SeqCst);
        loop {
            let Some(aligned) = next.checked_add(align - 1).map(|addr| addr & !(align - 1)) else {
                return null_mut();
            };
            let Some(end) = aligned.checked_add(size) else {
                return null_mut();
            };
            if end > self.heap_end {
                return null_mut();
            }
            match self
                .bump_next
                .compare_exchange(next, end, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return aligned as *mut u8,
                Err(current) => next = current,
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(core::mem::size_of::<FreeBlock>());

        let _guard = self.lock_free_list();
        let block = ptr.cast::<FreeBlock>();
        unsafe {
            block.write(FreeBlock {
                size,
                next: *self.free_list.get(),
            });
            *self.free_list.get() = block;
        }
    }
}

// ============================================================
// Tests
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    const HEAP_SIZE: usize = 4096;

    fn make_allocator() -> (FreeListAllocator, Vec<u8>) {
        let mut heap = vec![0u8; HEAP_SIZE];
        let start = heap.as_mut_ptr() as usize;
        let alloc = unsafe { FreeListAllocator::new(start, start + HEAP_SIZE) };
        (alloc, heap)
    }

    #[test]
    fn test_alloc_basic() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(32, 8).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_alloc_alignment() {
        let (alloc, _heap) = make_allocator();
        for align in [1, 2, 4, 8, 16] {
            let layout = Layout::from_size_align(8, align).unwrap();
            let ptr = unsafe { alloc.alloc(layout) };
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % align, 0, "align={align}");
        }
    }

    #[test]
    fn test_dealloc_and_reuse() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(64, 8).unwrap();

        let p1 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null());

        // After freeing, the next allocation should reuse the same block
        unsafe { alloc.dealloc(p1, layout) };
        let p2 = unsafe { alloc.alloc(layout) };
        assert!(!p2.is_null());
        assert_eq!(p1, p2, "should reuse the freed block");
    }

    #[test]
    fn test_multiple_alloc_dealloc() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(128, 8).unwrap();

        let p1 = unsafe { alloc.alloc(layout) };
        let p2 = unsafe { alloc.alloc(layout) };
        let p3 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null() && !p2.is_null() && !p3.is_null());

        unsafe { alloc.dealloc(p2, layout) };
        unsafe { alloc.dealloc(p1, layout) };

        let q1 = unsafe { alloc.alloc(layout) };
        let q2 = unsafe { alloc.alloc(layout) };
        assert!(!q1.is_null() && !q2.is_null());
    }

    #[test]
    fn test_first_fit_skips_small_blocks() {
        let (alloc, _heap) = make_allocator();
        let large = Layout::from_size_align(128, 8).unwrap();
        let small = Layout::from_size_align(16, 8).unwrap();
        let large_ptr = unsafe { alloc.alloc(large) };
        let small_ptr = unsafe { alloc.alloc(small) };
        unsafe {
            alloc.dealloc(large_ptr, large);
            alloc.dealloc(small_ptr, small);
        }
        assert_eq!(unsafe { alloc.alloc(large) }, large_ptr);
        assert_eq!(unsafe { alloc.alloc(small) }, small_ptr);
    }

    #[test]
    fn test_tiny_allocations_can_be_freed() {
        let (alloc, _heap) = make_allocator();
        let tiny = Layout::from_size_align(1, 1).unwrap();
        let first = unsafe { alloc.alloc(tiny) };
        let second = unsafe { alloc.alloc(tiny) };
        assert!(!first.is_null() && !second.is_null());
        assert!(first as usize + core::mem::size_of::<FreeBlock>() <= second as usize);
        unsafe {
            alloc.dealloc(first, tiny);
            alloc.dealloc(second, tiny);
        }
        assert_eq!(unsafe { alloc.alloc(tiny) }, second);
        assert_eq!(unsafe { alloc.alloc(tiny) }, first);
    }

    #[test]
    fn test_concurrent_dealloc_preserves_all_blocks() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(64, 8).unwrap();
        let addresses: Vec<_> = (0..32)
            .map(|_| unsafe { alloc.alloc(layout) } as usize)
            .collect();
        assert!(addresses.iter().all(|&addr| addr != 0));
        let barrier = std::sync::Barrier::new(addresses.len());
        std::thread::scope(|scope| {
            for &addr in &addresses {
                let alloc = &alloc;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    unsafe { alloc.dealloc(addr as *mut u8, layout) };
                });
            }
        });
        let mut reused: Vec<_> = (0..32)
            .map(|_| unsafe { alloc.alloc(layout) } as usize)
            .collect();
        let mut expected = addresses;
        reused.sort_unstable();
        expected.sort_unstable();
        assert_eq!(reused, expected);
    }

    #[test]
    fn test_oom() {
        let (alloc, _heap) = make_allocator();
        let layout = Layout::from_size_align(HEAP_SIZE + 1, 1).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(ptr.is_null(), "should return null when exceeding heap");
    }
}
