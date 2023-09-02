//! The panic handler and backtrace

use alloc::boxed::Box;

use crate::once_cell::race::OnceBox;
// use crate::sbi::shutdown;
use crate::sync::SMPSafeCell;
use crate::task::get_processor_id;
use core::panic::PanicInfo;


/// Control the concurrency of panic
pub static PANIC_LOCK: OnceBox<SMPSafeCell<bool>> = OnceBox::new();

use addr2line::Context;
use addr2line::gimli::{
    DebugAbbrev, DebugAddr, DebugAranges, DebugInfo, DebugLine, DebugLineStr, DebugRanges,
    DebugRngLists, DebugStr, DebugStrOffsets, EndianSlice, LittleEndian,
};


extern "C" {
    fn _rvbt_abbrev_start();
    fn _rvbt_abbrev_end();
    fn _rvbt_addr_start();
    fn _rvbt_addr_end();
    fn _rvbt_aranges_start();
    fn _rvbt_aranges_end();
    fn _rvbt_info_start();
    fn _rvbt_info_end();
    fn _rvbt_line_start();
    fn _rvbt_line_end();
    fn _rvbt_line_str_start();
    fn _rvbt_line_str_end();
    fn _rvbt_ranges_start();
    fn _rvbt_ranges_end();
    fn _rvbt_rnglists_start();
    fn _rvbt_rnglists_end();
    fn _rvbt_str_start();
    fn _rvbt_str_end();
    fn _rvbt_str_offsets_start();
    fn _rvbt_str_offsets_end();
}
fn _abbrev_section() -> DebugAbbrev<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_abbrev_start as usize;
    let end = _rvbt_abbrev_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugAbbrev::new(bytes, LittleEndian)
}

fn _addr_section() -> DebugAddr<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_addr_start as usize;
    let end = _rvbt_addr_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugAddr::from(EndianSlice::new(bytes, LittleEndian))
}
fn _aranges_section() -> DebugAranges<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_aranges_start as usize;
    let end = _rvbt_aranges_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugAranges::new(bytes, LittleEndian)
}
fn _info_section() -> DebugInfo<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_info_start as usize;
    let end = _rvbt_info_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugInfo::new(bytes, LittleEndian)
}
fn _line_section() -> DebugLine<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_line_start as usize;
    let end = _rvbt_line_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugLine::new(bytes, LittleEndian)
}
fn _line_str_section() -> DebugLineStr<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_line_str_start as usize;
    let end = _rvbt_line_str_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugLineStr::from(EndianSlice::new(bytes, LittleEndian))
}
fn _ranges_section() -> DebugRanges<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_ranges_start as usize;
    let end = _rvbt_ranges_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugRanges::new(bytes, LittleEndian)
}
fn _rnglists_section() -> DebugRngLists<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_rnglists_start as usize;
    let end = _rvbt_rnglists_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugRngLists::new(bytes, LittleEndian)
}
fn _str_section() -> DebugStr<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_str_start as usize;
    let end = _rvbt_str_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugStr::new(bytes, LittleEndian)
}
fn _str_offsets_section() -> DebugStrOffsets<EndianSlice<'static, LittleEndian>> {
    let start = _rvbt_str_offsets_start as usize;
    let end = _rvbt_str_offsets_end as usize;
    let bytes = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    DebugStrOffsets::from(EndianSlice::new(bytes, LittleEndian))
}

/// Initialize PANIC_LOCK
pub fn init_panic_lock() {
    let panic_lock = unsafe { SMPSafeCell::new(false) };
    if PANIC_LOCK.set(Box::new(panic_lock)).is_err() {
        println!("PANIC_LOCK has been initialized!");
    }
}

#[panic_handler]
/// panic handler
fn panic(info: &PanicInfo) -> ! {
    unsafe { riscv::register::sie::clear_stimer(); }
    if let Some(location) = info.location() {
        locked_println!(
            "[kernel] Hart {} panicked at {}:{} {}",
            get_processor_id(),
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        locked_println!("[kernel] Panicked: {}", info.message().unwrap());
    }
    backtrace2();
    loop {

    }
    // shutdown()
}
fn backtrace2() {
    backtrace();
    println!("return: {:#x}", backtrace2 as usize);
}
#[allow(unused)]
fn loader() {
    
}
/// backtrace function
pub fn backtrace() {
    
    // let guard = PANIC_LOCK.get().unwrap().exclusive_access();
    println!("---START BACKTRACE---");
    // let mut owned_dwarf: addr2line::gimli::Dwarf<alloc::vec::Vec<u8>> = addr2line::gimli::Dwarf::load(loader)?;
    let info = crate::trace::init_kernel_trace();
    let ctx = Context::from_sections(
        _abbrev_section(),
        _addr_section(),
        _aranges_section(),
        _info_section(),
        _line_section(),
        _line_str_section(),
        _ranges_section(),
        _rnglists_section(),
        _str_section(),
        _str_offsets_section(),
        EndianSlice::new(&[], LittleEndian)).unwrap();
    let mut _func_info = crate::rtrace::old_trace(&info);
    // func_info.iter().for_each(|x| {
        // println!("{}", x);
        // let frame = rvbt::frame::Frame { fp: (0), sp: (0), ra: (*x as u64) };
        // rvbt::symbol::resolve_frame(&frame, &|symbol| println!("{}", symbol));
    // });
    _func_info = crate::rtrace::my_trace(ctx);
    _func_info.iter().for_each(|x| {
        println!("{}", x);
        // let frame = rvbt::frame::Frame { fp: (0), sp: (0), ra: (*x as u64) };
        // rvbt::symbol::resolve_frame(&frame, &|symbol| println!("{}", symbol));
    });
    
    /*rvbt::frame::trace(&mut |frame| {
        println!("rvbt{:#x}", frame.ra);
        //rvbt::symbol::resolve_frame(frame, &|symbol| println!("{}", symbol));
        true
    });*/
    println!("---END   BACKTRACE---");
    // drop(guard);
    /*let mut fp: usize;
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
    println!("---END   BACKTRACE---");*/
}
