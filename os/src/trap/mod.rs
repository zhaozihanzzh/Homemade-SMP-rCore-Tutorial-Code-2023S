//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

mod context;

use crate::config::TRAMPOLINE;
use crate::syscall::syscall;
use crate::task::{
    check_signals_of_current, current_add_signal, current_trap_cx, current_trap_cx_user_va,
    current_user_token, exit_current_and_run_next, suspend_current_and_run_next, SignalFlags,
};
use crate::timer::{check_timer, set_next_trigger};
use core::arch::{asm, global_asm};
use riscv::register::sstatus::{self, Sstatus};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// Initialize trap handling
pub fn init() {
    set_kernel_trap_entry();
}
/// set trap entry for traps happen in kernel(supervisor) mode
fn set_kernel_trap_entry() {
    extern "C" {
        fn __s_alltraps();
    }
    unsafe {
        stvec::write(__s_alltraps as usize, TrapMode::Direct);
    }
}
/// set trap entry for traps happen in user mode
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

/// enable timer interrupt in supervisor mode
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

/// trap handler
#[no_mangle]
pub fn trap_handler() -> ! {
    if Sstatus::spp(&sstatus::read()) != sstatus::SPP::User {
        panic!("[DEBUG] Error: trap_handler: Sstatus::spp(&sstatus::read()) != sstatus::SPP::User");
    }
    set_kernel_trap_entry();
    let scause = scause::read();
    let stval = stval::read();
    // trace!("into {:?}", scause.cause());
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // get system call return value
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12], cx.x[13]]);
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            error!(
                "[kernel] trap_handler: {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            check_timer();
            suspend_current_and_run_next();
        }
        /*Trap::Interrupt(Interrupt::SupervisorSoft) => {
            let mut fp: usize;
            unsafe { asm!("mv {}, a0", out(reg) fp); }
            println!("S:a0={}", fp);
            // panic!("ZHAOZIHAN: Soft Interrupt!");
        }*/
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    // check signals
    if let Some((errno, msg)) = check_signals_of_current() {
        trace!("[kernel] trap_handler: .. check signals {}", msg);
        exit_current_and_run_next(errno);
    }
    trap_return();
}

/// return to user space
#[no_mangle]
pub fn trap_return() -> ! {
    //disable_supervisor_interrupt();
    set_user_trap_entry();
    let trap_cx_user_va = current_trap_cx_user_va();
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    // trace!("[kernel] trap_return: ..before return");
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",         // jump to new addr of __restore asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_user_va,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of usr page table
            options(noreturn)
        );
    }
}

/// handle trap from kernel
#[no_mangle]
pub fn trap_from_kernel() {
    // Read reg in inline asm
    unsafe {
        let mut fp: usize;
        asm!("mv {}, a0", out(reg) fp);
        println!("a0={}", fp);
        asm!("mv {}, sp", out(reg) fp);
        println!("From start: sp={:#x}", fp);
    }
    if Sstatus::spp(&sstatus::read()) != sstatus::SPP::Supervisor {
        panic!("[DEBUG] Error: trap_from_kernel: Sstatus::spp(&sstatus::read()) != sstatus::SPP::Supervisor");
    } else {
        println!("[DEBUG] trap_from_kernel: Sstatus::spp is Supervisor.");
    }
    use riscv::register::sepc;
    trace!("stval = {:#x}, sepc = {:#x}", stval::read(), sepc::read());
    //panic!("a trap {:?} from kernel!", scause::read().cause());
    let scause = scause::read();
    let stval = stval::read();
    // trace!("into {:?}", scause.cause());
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            println!("Z");
            // panic!("ZHAOZIHAN: Soft Interrupt!");
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            // check_timer();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            let sepc: usize;
            unsafe { asm!("ld {}, 33*8(sp)", out(reg) sepc); }
            panic!(
                "trap_from_kernel: {:?} in kernel, bad addr = {:#x}, bad instruction = {:#x}.",
                scause.cause(),
                stval,
                sepc,
            );
        }
        _ => {
            panic!(
                "trap_from_kernel: Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    unsafe {
        let mut fp: usize;
        asm!("mv {}, sp", out(reg) fp);
        println!("From end: sp={:#x}", fp);
    }
}

pub use context::TrapContext;
