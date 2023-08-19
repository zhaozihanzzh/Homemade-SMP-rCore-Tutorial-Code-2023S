//! SBI call wrappers

#![allow(unused)]

use core::arch::asm;
/// set timer sbi call id
const SBI_SET_TIMER: usize = 0;
/// console putchar sbi call id
const SBI_CONSOLE_PUTCHAR: usize = 1;
/// console getchar sbi call id
const SBI_CONSOLE_GETCHAR: usize = 2;
/// shutdown sbi call id
const SBI_SHUTDOWN: usize = 8;

/// general sbi call
#[inline(always)]
fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",     // sbi call
            inlateout("x10") arg0 => ret, // sbi call arg0 and return value
            in("x11") arg1, // sbi call arg1
            in("x12") arg2, // sbi call arg2
            in("x17") eid,// sbi call id
            in("x16") fid, // for sbi call id args need 2 reg (x16, x17)
        );
    }
    ret
}

/// use sbi call to set timer
pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, 0, timer, 0, 0);
}

/// use sbi call to putchar in console (qemu uart handler)
pub fn console_putchar(c: usize) {
    sbi_call(SBI_CONSOLE_PUTCHAR, 0, c, 0, 0);
}

/// use sbi call to getchar from console (qemu uart handler)
pub fn console_getchar() -> usize {
    sbi_call(SBI_CONSOLE_GETCHAR, 0, 0, 0, 0)
}

/// use sbi call to shutdown the kernel
pub fn shutdown() -> ! {
    sbi_call(SBI_SHUTDOWN, 0, 0, 0, 0);
    panic!("It should shutdown!");
}

/// use sbi call to start a hart
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> isize {
    sbi_call(0x48534D, 0,hartid, start_addr, opaque) as isize
}