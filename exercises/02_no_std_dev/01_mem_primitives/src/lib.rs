//! # no_std Memory Primitives
//!
//! In a `#![no_std]` environment, you have no standard library — only `core`.
//! These memory operation functions are the most fundamental building blocks in an OS kernel.
//! Functions like memcpy/memset in libc must be implemented by ourselves in bare-metal environments.
//!
//! ## Task
//!
//! Implement the following five functions:
//! - Only use the `core` crate, no `std`
//! - Do not call `core::ptr::copy`, `core::ptr::copy_nonoverlapping`, etc. (write your own loops)
//! - Handle edge cases correctly (n=0, overlapping memory regions, etc.)
//! - Pass all tests

// Force no_std in production; allow std in tests (cargo test framework requires it)
#![cfg_attr(not(test), no_std)]

/// Copy `n` bytes from `src` to `dst`.
///
/// - `dst` and `src` must not overlap (use `my_memmove` for overlapping regions)
/// - Returns `dst`
///
/// # Safety
/// `dst` and `src` must each point to at least `n` bytes of valid memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        unsafe { dst.add(i).write(src.add(i).read()) };
    }
    dst
}

/// Set `n` bytes starting at `dst` to the value `c`.
///
/// Returns `dst`.
///
/// # Safety
/// `dst` must point to at least `n` bytes of valid writable memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_memset(dst: *mut u8, c: u8, n: usize) -> *mut u8 {
    for i in 0..n {
        unsafe { dst.add(i).write(c) };
    }
    dst
}

/// Copy `n` bytes from `src` to `dst`, correctly handling overlapping memory.
///
/// Returns `dst`.
///
/// # Safety
/// `dst` and `src` must each point to at least `n` bytes of valid memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dst as usize) > (src as usize) {
        for i in (0..n).rev() {
            unsafe { dst.add(i).write(src.add(i).read()) };
        }
    } else {
        for i in 0..n {
            unsafe { dst.add(i).write(src.add(i).read()) };
        }
    }
    dst
}

/// Return the length of a null-terminated byte string, excluding the trailing null.
///
/// # Safety
/// `s` must point to a valid null-terminated byte string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_strlen(s: *const u8) -> usize {
    let mut len = 0;
    while unsafe { s.add(len).read() } != 0 {
        len += 1;
    }
    len
}

/// Compare two null-terminated byte strings.
///
/// Returns:
/// - `0`  : strings are equal
/// - `< 0`: `s1` is lexicographically less than `s2`
/// - `> 0`: `s1` is lexicographically greater than `s2`
///
/// # Safety
/// `s1` and `s2` must each point to a valid null-terminated byte string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let a = unsafe { s1.add(i).read() };
        let b = unsafe { s2.add(i).read() };
        if a != b || a == 0 {
            return i32::from(a) - i32::from(b);
        }
        i += 1;
    }
}

// ============================================================
// Tests (std is available under #[cfg(test)])
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memcpy_basic() {
        let src = [1u8, 2, 3, 4, 5];
        let mut dst = [0u8; 5];
        unsafe { my_memcpy(dst.as_mut_ptr(), src.as_ptr(), 5) };
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_zero_len() {
        let src = [0xFFu8; 4];
        let mut dst = [0u8; 4];
        unsafe { my_memcpy(dst.as_mut_ptr(), src.as_ptr(), 0) };
        assert_eq!(dst, [0u8; 4]);
    }

    #[test]
    fn test_memset_basic() {
        let mut buf = [0u8; 8];
        unsafe { my_memset(buf.as_mut_ptr(), 0xAB, 8) };
        assert!(buf.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn test_memset_partial() {
        let mut buf = [0u8; 8];
        unsafe { my_memset(buf.as_mut_ptr(), 0xFF, 4) };
        assert_eq!(&buf[..4], &[0xFF; 4]);
        assert_eq!(&buf[4..], &[0x00; 4]);
    }

    #[test]
    fn test_memmove_no_overlap() {
        let src = [1u8, 2, 3, 4];
        let mut dst = [0u8; 4];
        unsafe { my_memmove(dst.as_mut_ptr(), src.as_ptr(), 4) };
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memmove_overlap_forward() {
        // Copy buf[0..4] to buf[1..5], shifting right by 1
        let mut buf = [1u8, 2, 3, 4, 5];
        unsafe { my_memmove(buf.as_mut_ptr().add(1), buf.as_ptr(), 4) };
        assert_eq!(buf, [1, 1, 2, 3, 4]);
    }

    #[test]
    fn test_memmove_overlap_backward_and_self() {
        let mut buf = [1u8, 2, 3, 4, 5];
        let ptr = buf.as_mut_ptr();
        assert_eq!(unsafe { my_memmove(ptr, ptr.add(1), 4) }, ptr);
        assert_eq!(buf, [2, 3, 4, 5, 5]);
        unsafe { my_memmove(ptr, ptr, 5) };
        assert_eq!(buf, [2, 3, 4, 5, 5]);
    }

    #[test]
    fn test_zero_length_does_not_access_memory() {
        let dst = core::ptr::null_mut();
        let src = core::ptr::null();
        assert_eq!(unsafe { my_memcpy(dst, src, 0) }, dst);
        assert_eq!(unsafe { my_memset(dst, 1, 0) }, dst);
        assert_eq!(unsafe { my_memmove(dst, src, 0) }, dst);
    }

    #[test]
    fn test_strcmp_prefix_and_unsigned_bytes() {
        assert!(unsafe { my_strcmp(b"a\0".as_ptr(), b"ab\0".as_ptr()) } < 0);
        assert!(unsafe { my_strcmp([0xFF, 0].as_ptr(), [0x7F, 0].as_ptr()) } > 0);
    }

    #[test]
    fn test_strlen_basic() {
        let s = b"hello\0";
        assert_eq!(unsafe { my_strlen(s.as_ptr()) }, 5);
    }

    #[test]
    fn test_strlen_empty() {
        let s = b"\0";
        assert_eq!(unsafe { my_strlen(s.as_ptr()) }, 0);
    }

    #[test]
    fn test_strcmp_equal() {
        let a = b"hello\0";
        let b = b"hello\0";
        assert_eq!(unsafe { my_strcmp(a.as_ptr(), b.as_ptr()) }, 0);
    }

    #[test]
    fn test_strcmp_less() {
        let a = b"abc\0";
        let b = b"abd\0";
        assert!(unsafe { my_strcmp(a.as_ptr(), b.as_ptr()) } < 0);
    }

    #[test]
    fn test_strcmp_greater() {
        let a = b"abd\0";
        let b = b"abc\0";
        assert!(unsafe { my_strcmp(a.as_ptr(), b.as_ptr()) } > 0);
    }
}
