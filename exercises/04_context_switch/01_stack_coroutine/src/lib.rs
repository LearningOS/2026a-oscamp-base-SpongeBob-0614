//! # Stackful Coroutine and Context Switch (riscv64)
//!
//! In this exercise, you implement the minimal context switch using inline assembly,
//! which is the core mechanism of OS thread scheduling. This crate is **riscv64 only**;
//! run `cargo test` on riscv64 Linux, or use the repo's normal flow (`./check.sh` / `oscamp`) on x86 with QEMU.
//!
//! ## Key Concepts
//! - **Callee-saved registers**: Save and restore them on switch so the switched-away task can resume correctly later.
//! - **Stack pointer `sp`** and **return address `ra`**: Restore them in the new context; the first time we switch to a task, `ret` jumps to `ra` (the entry point).
//! - Naked assembly: `core::arch::naked_asm!`
//!
//! ## riscv64 ABI (for this exercise)
//! - Callee-saved: `sp`, `ra`, `s0`–`s11`, and hard-float ABI registers `fs0`–`fs11`.
//! - The `ret` instruction is `jalr zero, 0(ra)`.
//! - First and second arguments: `a0` (old context), `a1` (new context).

#![cfg(target_arch = "riscv64")]

/// Saved register state for one task (riscv64). Layout must match the offsets used in the asm below:
/// `sp` at 0, `ra` at 8, `s0`–`s11` at 16, 24, … 104,
/// and `fs0`–`fs11` at 112, 120, … 200 (stored as raw floating-point bits).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskContext {
    pub sp: u64,
    pub ra: u64,
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
    pub fs0: u64,
    pub fs1: u64,
    pub fs2: u64,
    pub fs3: u64,
    pub fs4: u64,
    pub fs5: u64,
    pub fs6: u64,
    pub fs7: u64,
    pub fs8: u64,
    pub fs9: u64,
    pub fs10: u64,
    pub fs11: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            sp: 0,
            ra: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            fs0: 0,
            fs1: 0,
            fs2: 0,
            fs3: 0,
            fs4: 0,
            fs5: 0,
            fs6: 0,
            fs7: 0,
            fs8: 0,
            fs9: 0,
            fs10: 0,
            fs11: 0,
        }
    }

    /// Initialize this context so that when we switch to it, execution starts at `entry`.
    ///
    /// - Set `ra = entry` so that the first `ret` in the new context jumps to `entry`.
    /// - Set `sp = stack_top` with 16-byte alignment (RISC-V ABI requires 16-byte aligned stack at function entry).
    /// - Leave the saved integer and floating-point registers zero.
    pub fn init(&mut self, stack_top: usize, entry: usize) {
        *self = Self {
            sp: (stack_top & !15) as u64,
            ra: entry as u64,
            ..Self::empty()
        };
    }
}

/// Switch from `old` to `new` context: save current callee-saved regs into `old`, load from `new`, then `ret` (jumps to `new.ra`).
///
/// In asm: store `sp`, `ra`, `s0`–`s11`, `fs0`–`fs11` to `[a0]` (old), load from `[a1]` (new), zero `a0`/`a1` so we do not leak pointers into the new context, then `ret`.
///
/// Must be `#[unsafe(naked)]` to prevent the compiler from generating a prologue/epilogue.
///
/// # Safety
/// Both pointers must be valid and distinct. `new` must contain a suspended context
/// or an entry point and live, writable stack.
/// Both contexts and their stacks must remain alive until their tasks finish.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(_old: *mut TaskContext, _new: *const TaskContext) {
    core::arch::naked_asm!(
        // Naked assembly does not inherit the target's floating-point features.
        ".option push",
        ".option arch, +d",
        "sd sp, 0(a0)",
        "sd ra, 8(a0)",
        "sd s0, 16(a0)",
        "sd s1, 24(a0)",
        "sd s2, 32(a0)",
        "sd s3, 40(a0)",
        "sd s4, 48(a0)",
        "sd s5, 56(a0)",
        "sd s6, 64(a0)",
        "sd s7, 72(a0)",
        "sd s8, 80(a0)",
        "sd s9, 88(a0)",
        "sd s10, 96(a0)",
        "sd s11, 104(a0)",
        "fsd fs0, 112(a0)",
        "fsd fs1, 120(a0)",
        "fsd fs2, 128(a0)",
        "fsd fs3, 136(a0)",
        "fsd fs4, 144(a0)",
        "fsd fs5, 152(a0)",
        "fsd fs6, 160(a0)",
        "fsd fs7, 168(a0)",
        "fsd fs8, 176(a0)",
        "fsd fs9, 184(a0)",
        "fsd fs10, 192(a0)",
        "fsd fs11, 200(a0)",
        "ld sp, 0(a1)",
        "ld ra, 8(a1)",
        "ld s0, 16(a1)",
        "ld s1, 24(a1)",
        "ld s2, 32(a1)",
        "ld s3, 40(a1)",
        "ld s4, 48(a1)",
        "ld s5, 56(a1)",
        "ld s6, 64(a1)",
        "ld s7, 72(a1)",
        "ld s8, 80(a1)",
        "ld s9, 88(a1)",
        "ld s10, 96(a1)",
        "ld s11, 104(a1)",
        "fld fs0, 112(a1)",
        "fld fs1, 120(a1)",
        "fld fs2, 128(a1)",
        "fld fs3, 136(a1)",
        "fld fs4, 144(a1)",
        "fld fs5, 152(a1)",
        "fld fs6, 160(a1)",
        "fld fs7, 168(a1)",
        "fld fs8, 176(a1)",
        "fld fs9, 184(a1)",
        "fld fs10, 192(a1)",
        "fld fs11, 200(a1)",
        "li a0, 0",
        "li a1, 0",
        "ret",
        ".option pop",
    );
}

const STACK_SIZE: usize = 1024 * 64;

/// Allocate a stack for a coroutine. Returns `(buffer, stack_top)` where `stack_top` is the high address
/// (stack grows down). The buffer must be kept alive for the lifetime of the context using this stack.
pub fn alloc_stack() -> (Vec<u8>, usize) {
    let buffer = vec![0; STACK_SIZE];
    let stack_top = (buffer.as_ptr() as usize + buffer.len()) & !15;
    (buffer, stack_top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    extern "C" fn task_entry() {
        COUNTER.store(42, Ordering::SeqCst);
        loop {
            std::hint::spin_loop();
        }
    }

    #[test]
    fn test_alloc_stack() {
        let (buf, top) = alloc_stack();
        assert_eq!(top, buf.as_ptr() as usize + STACK_SIZE);
        assert!(top % 16 == 0);
    }

    #[test]
    fn test_context_init() {
        let (buf, top) = alloc_stack();
        let _ = buf;
        let mut ctx = TaskContext::empty();
        let entry = task_entry as *const () as usize;
        ctx.init(top, entry);
        assert_eq!(ctx.ra, entry as u64);
        assert!(ctx.sp != 0);
    }

    #[test]
    fn test_context_init_aligns_and_clears_registers() {
        let mut ctx = TaskContext::empty();
        ctx.s0 = 123;
        ctx.s11 = 456;
        ctx.fs0 = 789;
        ctx.fs11 = 123;
        ctx.init(0x10ff, 0x2000);
        assert_eq!(ctx.sp, 0x10f0);
        assert_eq!(ctx.ra, 0x2000);
        assert_eq!(ctx.s0, 0);
        assert_eq!(ctx.s11, 0);
        assert_eq!(ctx.fs0, 0);
        assert_eq!(ctx.fs11, 0);
    }

    #[test]
    fn test_switch_to_task() {
        COUNTER.store(0, Ordering::SeqCst);

        static mut MAIN_CTX_PTR: *mut TaskContext = std::ptr::null_mut();
        static mut TASK_CTX_PTR: *mut TaskContext = std::ptr::null_mut();

        extern "C" fn cooperative_task() {
            COUNTER.store(99, Ordering::SeqCst);
            unsafe {
                core::arch::asm!(
                    "fmv.d.x fs0, {first}",
                    "fmv.d.x fs11, {last}",
                    first = in(reg) 0x1234_u64,
                    last = in(reg) 0x5678_u64,
                    out("fs0") _,
                    out("fs11") _,
                    options(nostack),
                );
                switch_context(TASK_CTX_PTR, MAIN_CTX_PTR);
            }
            // An entry context has no caller to return to. Also keep the switch
            // from becoming a tail call that restores the entry's FP registers.
            std::process::abort();
        }

        let (_stack_buf, stack_top) = alloc_stack();
        let mut main_ctx = TaskContext::empty();
        let mut task_ctx = TaskContext::empty();
        task_ctx.init(stack_top, cooperative_task as *const () as usize);

        unsafe {
            MAIN_CTX_PTR = &raw mut main_ctx;
            TASK_CTX_PTR = &raw mut task_ctx;
            core::arch::asm!(
                "fmv.d.x fs0, {first}",
                "fmv.d.x fs11, {last}",
                first = in(reg) 0x9abc_u64,
                last = in(reg) 0xdef0_u64,
                out("fs0") _,
                out("fs11") _,
                options(nostack),
            );
            switch_context(&raw mut main_ctx, &raw const task_ctx);
        }

        assert_eq!(COUNTER.load(Ordering::SeqCst), 99);
        let (first, last): (u64, u64);
        unsafe {
            core::arch::asm!(
                "fmv.x.d {first}, fs0",
                "fmv.x.d {last}, fs11",
                first = out(reg) first,
                last = out(reg) last,
                options(nostack),
            );
        }
        assert_eq!((first, last), (0x9abc, 0xdef0));
        assert_eq!((main_ctx.fs0, main_ctx.fs11), (0x9abc, 0xdef0));
        assert_eq!((task_ctx.fs0, task_ctx.fs11), (0x1234, 0x5678));
    }
}
