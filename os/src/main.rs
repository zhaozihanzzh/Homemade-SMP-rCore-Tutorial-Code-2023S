//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//! - [`mm`]: Address map using SV39
//! - [`sync`]: Wrap a static data structure inside it so that we are able to access it without any `unsafe`.
//! - [`fs`]: Separate user from file system with some structures
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_tasks()`] and for the first time go to
//! userspace.

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
mod device_tree;
pub mod config;
pub mod drivers;
pub mod fs;
pub mod lang_items;
pub mod logging;
pub mod mm;
pub mod sbi;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod timer;
pub mod trap;

use core::arch::global_asm;

use riscv::register::sstatus::{Sstatus, self};
use rvbt::{frame::trace, symbol::resolve_frame, init::debug_init};
use crate::mm::KERNEL_SPACE;

global_asm!(include_str!("entry.asm"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

extern "C" {
    fn _start_backup_hart();
}

#[no_mangle]
/// the rust entry-point of os
pub fn rust_main(hart_id: usize, dtb: usize) -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    println!("Boot from CPU {}", hart_id);
    let board_info = device_tree::parse(dtb);
    println!("SMP total {} harts", board_info.smp);
    logging::init();
    mm::init();
    mm::remap_test();
    debug_init();
    trace(&mut |frame| {
        resolve_frame(frame, &|symbol| println!("{}", symbol));
        true
    });
    
    for i in 0..board_info.smp {
        if i != hart_id {
            println!("hart_start {}", sbi::hart_start(i, _start_backup_hart as usize, i));
        }
    }
    // unsafe { sstatus::set_spp(SPP::Supervisor); }
    trap::init();
    // trap::enable_ipi();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    println!("SIE status: {}", Sstatus::sie(&sstatus::read()));
    println!("SPP status: {}", Sstatus::spp(&sstatus::read()) as usize);
    
    fs::list_apps();
    task::add_initproc();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

#[no_mangle]
fn start_backup_hart(hart_id: usize, hart_id2: usize) -> ! {
    println!("Boot from backup CPU {} {}", hart_id, hart_id2);
    KERNEL_SPACE.exclusive_access().activate();
    // debug_init();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::add_initproc();
    task::run_tasks();
    loop {
        unsafe { core::arch::asm!("nop"); }
    }
}