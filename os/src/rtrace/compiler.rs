use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bit_field::BitField;
use core::arch::asm;
use log::trace;

use crate::mm::{KERNEL_SPACE, VirtAddr};

pub trait Symbol {
    fn addr(&self) -> usize;
    fn name(&self) -> &str;
}

// 在函数第一条指令，开辟栈空间
// 指令 addi sp,sp,imm 4字节
// imm[11:0] rs 000 rd 0010011
// 有三种二字节的压缩指令
// c.addi rd,imm
// 000 [imm5] rd [imm4-0] 01
// c.addi16sp imm
// 011 [imm9] 00010 [imm4|6|8|7|5] 01

enum InstructionSp {
    Addi(u32),
    CAddi(u32),
    CAddi16Sp(u32),
    Unknown,
}

impl InstructionSp {
    fn new(ins: u32) -> Self {
        let opcode = ins.get_bits(0..7);
        match opcode {
            0b0010011 => {
                // 高12位符号扩展
                let mut imm = ins.get_bits(20..32);
                for i in 12..32 {
                    imm.set_bit(i, imm.get_bit(11));
                }
                let imm = imm as i32;
                assert!(imm < 0);
                InstructionSp::Addi((-imm) as u32)
            }
            _ => {
                let short_ins = ins.get_bits(0..16);
                let high = short_ins.get_bits(13..16);
                let low = short_ins.get_bits(0..2);
                match (high, low) {
                    (0b000, 0b01) => {
                        // 保证是sp
                        let rd = short_ins.get_bits(7..12);
                        if rd != 2 {
                            return InstructionSp::Unknown;
                        }
                        let mut imm = 0;
                        imm.set_bits(0..5, short_ins.get_bits(2..7));
                        imm.set_bit(5, short_ins.get_bit(12));
                        // 符号扩展
                        for i in 6..32 {
                            imm.set_bit(i, imm.get_bit(5));
                        }
                        let imm = imm as i32;
                        trace!("[CADDI] {:#b}", imm);
                        assert!(imm < 0);
                        InstructionSp::CAddi((-imm) as u32)
                    }
                    (0b011, 0b01) => {
                        let flag = short_ins.get_bits(7..=11);
                        if flag != 0b00010 {
                            return InstructionSp::Unknown;
                        }
                        let mut imm = 0u32;
                        imm.set_bit(9, short_ins.get_bit(12));
                        imm.set_bit(8, short_ins.get_bit(4));
                        imm.set_bit(7, short_ins.get_bit(3));
                        imm.set_bit(6, short_ins.get_bit(5));
                        imm.set_bit(5, short_ins.get_bit(2));
                        imm.set_bit(4, short_ins.get_bit(6));
                        for i in 10..32 {
                            imm.set_bit(i, imm.get_bit(9));
                        }
                        let imm = imm as i32;
                        trace!("sp_size: {}", -imm);
                        assert!(imm < 0);
                        InstructionSp::CAddi16Sp((-imm) as u32)
                    }
                    _ => InstructionSp::Unknown,
                }
            }
        }
    }
}

fn sd_ra(ins: u32) -> Option<u32> {
    // 检查指令是否是存储ra
    let opcode = ins.get_bits(0..7);
    return match opcode {
        0b0100011 => {
            // 四字节的sd指令
            let func = ins.get_bits(12..=14);
            if func != 0b011 {
                return None;
            }
            let rd = ins.get_bits(15..=19); //sp
            let rt = ins.get_bits(20..=24); //ra
            if rd != 2 || rt != 1 {
                return None;
            }
            let mut imm = 0u32;
            imm.set_bits(0..=4, ins.get_bits(7..=11));
            imm.set_bits(5..=11, ins.get_bits(25..=31));
            for i in 12..32 {
                imm.set_bit(i, imm.get_bit(11));
            }
            let imm = imm as isize;
            assert!(imm > 0);
            Some(imm as u32)
        }
        _ => {
            // 2字节的sd指令
            // c.sdsp
            // 111 [uimm5:3 8:6] rt 10
            let short_ins = ins.get_bits(0..16);
            let high = short_ins.get_bits(13..16);
            let low = short_ins.get_bits(0..2);
            match (high, low) {
                (0b111, 0b10) => {
                    let mut imm = 0u32;
                    imm.set_bits(3..6, short_ins.get_bits(10..13));
                    imm.set_bits(6..9, short_ins.get_bits(7..10));
                    Some(imm)
                }
                (_, _) => None,
            }
        }
    };
}
pub fn old_trace<T: Symbol>(symbol: &Vec<T>) -> Vec<String> {
    let s = old_trace::<T> as usize;
    // 函数的第一条指令
    let mut ins = s as *const u32;
    let mut sp = unsafe {
        let t: usize;
        asm!("mv {},sp",out(reg)t);
        t
    };
    let mut ans_str = Vec::new();
    loop {
        let first_ins = unsafe {ins.read_volatile()};
        trace!(
            "first_ins: {:#x} {:#b}",
            first_ins.get_bits(0..16),
            first_ins.get_bits(0..16)
        );
        let ans = InstructionSp::new(first_ins);
        let (next_ins, size) = match ans {
            InstructionSp::Addi(size) => unsafe {
                // 四字节指令
                (ins.add(1).read_volatile(), size)
            }
            InstructionSp::CAddi(size) | InstructionSp::CAddi16Sp(size) => unsafe {
                // 双字节指令
                let ins = (ins as *const u16).add(1) as *const u32;
                (ins.read_volatile(), size)
            }
            InstructionSp::Unknown => {
                // 未知指令
                break;
            }
        };
        // 第二条指令就是记录有ra的值
        // 需要确保第二条指令是否是存储ra
        if sd_ra(next_ins).is_none() {
            break;
        }
        let stack_size = size;
        let ra_addr = sp + stack_size as usize - 8;
        let ra = unsafe {(ra_addr as *const usize).read_volatile()}; //8字节存储
        let mut symbol_flag = false;
        let mut _new_range_end = 0;
        
        trace!("ra: {:#x}", ra);
        println!("Stack size {}, ra {:#x}, sp {:#x}, ins {:#x}", stack_size, ra, sp, ins as usize);
        let mut _symbol_addr = 0;
        for i in 0..symbol.len() {
            if symbol[i].addr() == ra
                || (i + 1 < symbol.len() && (symbol[i].addr()..symbol[i + 1].addr()).contains(&ra))
            {
                let str = format!(
                    "{:#x} (+{}) {}",
                    symbol[i].addr(),
                    ra - symbol[i].addr(),
                    symbol[i].name()
                );
                println!("{}", str);
                _symbol_addr = symbol[i].addr();
                ins = symbol[i].addr() as *const u32;
                symbol_flag = true;
                ans_str.push(str.clone());
                break;
            }
        }
        if !symbol_flag {
            break;
            // println!("Error: it should stop!");
        }
        sp += stack_size as usize;
        // ins = new_ins as *const u32;
    }
    ans_str
}
pub fn my_trace(ctx: addr2line::Context::<addr2line::gimli::EndianSlice<'_, addr2line::gimli::LittleEndian>>) -> Vec<String> {
    let s = my_trace as usize;
    // 函数的第一条指令
    let mut ins = s as *const u32;
    let mut sp = unsafe {
        let t: usize;
        asm!("mv {},sp",out(reg)t);
        t
    };
    let mut _ans_str = Vec::new();
    
    loop {
        let first_ins = unsafe {ins.read_volatile()};
        println!(
            "first_ins: {:#x} {:#b}",
            first_ins.get_bits(0..16),
            first_ins.get_bits(0..16)
        );
        let ans = InstructionSp::new(first_ins);
        let (next_ins, size) = match ans {
            InstructionSp::Addi(size) => unsafe {
                // 四字节指令
                (ins.add(1).read_volatile(), size)
            }
            InstructionSp::CAddi(size) | InstructionSp::CAddi16Sp(size) => unsafe {
                // 双字节指令
                let ins = (ins as *const u16).add(1) as *const u32;
                (ins.read_volatile(), size)
            }
            InstructionSp::Unknown => {
                // 未知指令
                break;
            }
        };
        // 第二条指令就是记录有ra的值
        // 需要确保第二条指令是否是存储ra
        if sd_ra(next_ins).is_none() {
            break;
        }
        let stack_size = size;
        let ra_addr = sp + stack_size as usize - 8;
        let ra = unsafe {(ra_addr as *const usize).read_volatile()}; //8字节存储
        let mut flag = true;
        // let mut symbol_flag = false;
        let mut new_ins = ins;
        let mut _new_range_end = 0;

        trace!("ra: {:#x}", ra);
        

        println!("Outer loop: stack_size={:#x}, ra={:#x}, sp={:#x}, ins={:#x}", stack_size, ra, sp, ins as usize);
        match KERNEL_SPACE.exclusive_access().get().translate(VirtAddr::from(ra).floor()) {
            Some(pte) => {
                if !(pte.is_valid() && pte.readable() && pte.executable() && (pte.flags().bits() & (1 << 4) == 0)) {
                    println!("ERROR: invalid or unreadable RA!");
                }
            },
            None => {println!("ERROR: translate err RA!");},
        }
        match KERNEL_SPACE.exclusive_access().get().translate(VirtAddr::from(sp).floor()) {
            Some(pte) => {
                if !(pte.is_valid() && pte.readable() && pte.writable() && (pte.flags().bits() & (1 << 4) == 0)) {
                    println!("ERROR: invalid or unreadable SP!");
                }
            },
            None => {println!("ERROR: translate err SP!");},
        }
        /*match ctx.find_dwarf_and_unit(ra as u64) {
            crate::addr2line::LookupResult::Load { load: _, continuation: _ } => { println!("Error: load split dwarf needed!"); },
            crate::addr2line::LookupResult::Output(result) => { println!("before match"); match result {
                Some(dwarf) => { dwarf.1.},
                None => todo!(),
            }}
        }*/
        let (file, line, col) = match ctx.find_location(ra as u64) {
            Ok(location) => match location {
                Some(location) => (location.file.unwrap_or("UnknownFile"), location.line.unwrap_or(0), location.column.unwrap_or(0)),
                None => { /*flag = false;*/ println!("Can't find location!"); ("UnknownFile", 0, 0) }
            },
            Err(_) => { println!("Reached Err");  ("UnknownFile", 0, 0) }
        };
        match ctx.find_frames(ra as u64) {
            addr2line::LookupResult::Load { load: _, continuation: _ } => {
                println!("Error: load split dwarf needed!");
            },
            addr2line::LookupResult::Output(result) => { println!("before match"); match result {
                Ok(mut iter) => {
                    println!("Before inner loop");
                    loop {
                        match iter.next() {
                            Ok(option) => match option {
                                Some(frame) => {
                                    match frame.range {
                                        Some(r) => {
                                            _new_range_end = r.end;
                                            println!("Range {:#x} {:#x}", r.begin, r.end);
                                            if !(r.begin.._new_range_end).contains(&(ra as u64)) || r.begin == 0 {
                                                println!("Warn: flag changes to false!");
                                                flag = false;
                                                break;
                                            } else {
                                                new_ins = r.begin as *const u32;
                                            }
                                        },
                                        None => {println!("~");},
                                    }
                                    match frame.dw_die_offset {
                                        Some(offset) =>  println!("Func offset {}", offset.0),
                                        None => {println!("@");},
                                    }
                                    match frame.location {
                                        Some(loc) => {
                                            match loc.file {
                                                Some(file_name) => println!("Func file name {}", file_name),
                                                None => {println!("!");},
                                            };
                                            match loc.line {
                                                Some(line) => println!("Func line {}", line),
                                                None => {println!("$");},
                                            };
                                            match loc.column {
                                                Some(column) => println!("Func column {}", column),
                                                None => {println!("^");},
                                            }                                            
                                        },
                                        None => {println!("%");},
                                    }
                                    match frame.function {
                                        Some(name) => {
                                            match name.demangle() {
                                                Ok(str) => println!("Func name {}", str),
                                                Err(_) => {println!("&");},
                                            }
                                        },
                                        None => {println!("`");}, // We must break here
                                    }
                                },
                                None => {println!("*"); break;},
                            },
                            Err(_) => {
                                println!("Error: can't lookup function name from frame!");
                                // break;
                            },
                        }

                    }
                },
                Err(_) => { println!("Error: lookup result invalid") },}
            }
        };
        
        
        println!("{}:{} at {}, ra = {:#x}", file, line, col, ra);
        
        /*if flag != symbol_flag {
            println!("Error: flag {} mismatches symbol_flag {}", flag, symbol_flag);
        }*/
        // 如果找不到符号，停止。必须要有这个机制
        if !flag {
            println!("Stop through flag");
            break;
        }
        
        /*if symbol_addr != new_ins as usize {
            println!("Error: address is not the same: {:#x} {:#x}!", symbol_addr, new_ins);
        }*/
        sp += stack_size as usize;
        ins = new_ins as *const u32;
    }
    _ans_str
}
 
