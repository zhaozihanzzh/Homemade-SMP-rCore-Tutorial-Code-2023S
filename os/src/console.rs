//! SBI console driver, for text output
use crate::{sbi::console_putchar, sync::SMPSafeCell, once_cell::race::OnceBox, task::get_processor_id};
use alloc::boxed::Box;
use core::fmt::{self, Write};

struct Stdout;


/// Control the concurrency of panic
pub static CONSOLE_LOCK: OnceBox<SMPSafeCell<bool>> = OnceBox::new();

impl Write for Stdout {
    /// write str to console
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _lock = CONSOLE_LOCK.get().unwrap().exclusive_access();
        let processor_id = get_processor_id();
        console_putchar('\x1b' as usize);
        console_putchar('[' as usize);
        if processor_id == 0 {
            console_putchar('3' as usize);
            console_putchar('1' as usize);
        } else if processor_id == 1 {
            console_putchar('9' as usize);
            console_putchar('3' as usize);
        } else if processor_id == 2 {
            console_putchar('3' as usize);
            console_putchar('4' as usize);
        } else if processor_id == 3 {
            console_putchar('3' as usize);
            console_putchar('2' as usize);
        }
        console_putchar('m' as usize);
        for c in s.chars() {
            console_putchar(c as usize);
        }
        console_putchar('\x1b' as usize);
        console_putchar('[' as usize);
        console_putchar('0' as usize);
        console_putchar('m' as usize);
        Ok(())
    }
}
/// print to the host console using the format string and arguments.
pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// Print! macro to the host console using the format string and arguments.
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

/// Println! macro to the host console using the format string and arguments.
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

/// print with SMP lock
pub fn locked_print(args: fmt::Arguments) {
    // let _lock = CONSOLE_LOCK.get().unwrap().exclusive_access();
    Stdout.write_fmt(args).unwrap();
}

pub fn init_locked_print() {
    let b: Box<SMPSafeCell<bool>> = Box::new(unsafe {SMPSafeCell::new(false)});
    if CONSOLE_LOCK.set(b).is_err() {
        println!("CONSOLE_LOCK has been initialized!");
    }
}

/// Println! macro for concurrency.
#[macro_export]
macro_rules! locked_println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::locked_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
