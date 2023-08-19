//! The panic handler and backtrace

use lazy_static::lazy_static;

use crate::sbi::shutdown;
use crate::sync::SMPSafeCell;
use crate::task::current_kstack_top;
use core::arch::asm;
use core::panic::PanicInfo;

lazy_static! {
    /// Control the concurrency of panic
    pub static ref PANIC_LOCK: SMPSafeCell<bool> = unsafe { SMPSafeCell::new(false) };
}
#[panic_handler]
/// panic handler
fn panic(info: &PanicInfo) -> ! {
    // unsafe { riscv::register::sie::clear_stimer(); }
    // let guard = PANIC_LOCK.exclusive_access();
    if let Some(location) = info.location() {
        println!(
            "[kernel] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        println!("[kernel] Panicked: {}", info.message().unwrap());
    }
    // drop(guard);
    rvbt::frame::trace(&mut |frame| {
        rvbt::symbol::resolve_frame(frame, &|symbol| println!("{}", symbol));
        true
    });
    unsafe {
        backtrace();
    }
    shutdown()
}
/// backtrace function
#[allow(unused)]
unsafe fn backtrace() {
    let mut fp: usize;
    let stop = current_kstack_top();
    asm!("mv {}, s0", out(reg) fp);
    println!("---START BACKTRACE---");
    for i in 0..10 {
        if fp == stop {
            break;
        }
        println!("#{}:ra={:#x}", i, *((fp - 8) as *const usize));
        fp = *((fp - 16) as *const usize);
    }
    println!("---END   BACKTRACE---");
}
