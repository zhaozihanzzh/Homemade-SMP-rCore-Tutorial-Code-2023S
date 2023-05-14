//! File and filesystem-related syscalls
use crate::fs::{make_pipe, open_file, OpenFlags, Stat, create_link, delete_link, get_link_count, OSInode, StatMode};
use crate::mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer};
use crate::task::{current_process, current_task, current_user_token};
use alloc::sync::Arc;
/// write syscall
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_write",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}
/// read syscall
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_read",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}
/// open sys
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!(
        "kernel:pid[{}] sys_open",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}
/// close syscall
pub fn sys_close(fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_close",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}
/// pipe syscall
pub fn sys_pipe(pipe: *mut usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_pipe",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}
/// dup syscall
pub fn sys_dup(fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_dup",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let task = current_task().unwrap().process.upgrade().unwrap();
    let inner = task.inner_exclusive_access();
    if _fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[_fd] {
        if let Some(inode) = file.clone().as_any().downcast_ref::<OSInode>() {
            let mut mode: StatMode = StatMode::FILE;
            if inode.get_inode_is_dir() {
                mode = StatMode::DIR;
            }
            let inode_id =inode.get_inode_id();
            let tmp_stat = Stat::new(0, inode_id as u64, mode, get_link_count(inode_id));
            drop(inner); // must drop in advance to pass the test
            let mut remain_len = core::mem::size_of::<Stat>();
            let buffers = translated_byte_buffer(current_user_token(), _st as *const u8, remain_len);
            let mut tmp_stat_ptr = &tmp_stat as *const Stat as *const u8;
            for buffer in buffers {
                // if buffer.len() <= remain_len {
                    for byte in buffer {
                        unsafe {
                            *byte = *tmp_stat_ptr;
                            tmp_stat_ptr = tmp_stat_ptr.add(1);
                        }
                        remain_len -= 1;
                    }
                // }
            }
            return 0;
        }
        // release current task TCB manually to avoid multi-borrow
        //drop(inner);
    }
    -1
}

const NAME_LENGTH_LIMIT: usize = 27;
/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let old_name_buffers = translated_byte_buffer(current_user_token(), _old_name as *const u8, NAME_LENGTH_LIMIT);
    let mut old_name = [0u8; NAME_LENGTH_LIMIT + 1];
    let mut location = 0;
    let mut has_reached_zero = false;
    for buffer in old_name_buffers {
        if has_reached_zero {
            break;
        }
        for byte in buffer {
            old_name[location] = *byte;
            if *byte == 0 {
                has_reached_zero = true;
                break;
            }
            location = location + 1;
        }
    }
    let old_name_len = location;
    let new_name_buffers = translated_byte_buffer(current_user_token(), _new_name as *const u8, NAME_LENGTH_LIMIT);
    let mut new_name = [0u8; NAME_LENGTH_LIMIT + 1];
    location = 0;
    has_reached_zero = false;
    for buffer in new_name_buffers {
        if has_reached_zero {
            break;
        }
        for byte in buffer {
            new_name[location] = *byte;
            if *byte == 0 {
                has_reached_zero = true;
                break;
            }
            location = location + 1;
        }
    }
    let new_name_len = location;
    // Check if we _old_name == _new_name
    let mut is_not_same = false;
    for location in 0..NAME_LENGTH_LIMIT {
        if old_name[location] != new_name[location] {
            is_not_same = true;
            break;
        }
        if old_name[location] == 0 {
            break;
        }
    }
    if !is_not_same {
        println!("Link same file: {}, {}", core::str::from_utf8(&old_name[..old_name_len]).unwrap(), core::str::from_utf8(&new_name[..new_name_len]).unwrap());
        return -1;
    }
    // Note: in rust *&str should not contain a zero in the end
    create_link(core::str::from_utf8(&old_name[..old_name_len]).unwrap(), core::str::from_utf8(&new_name[..new_name_len]).unwrap());
    0
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let name_buffers = translated_byte_buffer(current_user_token(), _name as *const u8, NAME_LENGTH_LIMIT);
    let mut name = [0u8; NAME_LENGTH_LIMIT + 1];
    let mut location = 0;
    let mut has_reached_zero = false;
    for buffer in name_buffers {
        if has_reached_zero {
            break;
        }
        for byte in buffer {
            name[location] = *byte;
            if *byte == 0 {
                has_reached_zero = true;
                break;
            }
            location = location + 1;
        }
    }
    println!("Prepare to delete link for {}", core::str::from_utf8(&name[..location]).unwrap());
    delete_link(core::str::from_utf8(&name[..location]).unwrap())
}
