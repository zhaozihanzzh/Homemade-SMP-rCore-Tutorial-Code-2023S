//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    loader::get_app_data_by_name,
    mm::{translated_refmut, translated_str, translated_byte_buffer, VirtAddr, MapPermission},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,
        write_current_syscall_times_array,
        get_current_start_running_time, 
    },
    timer::get_time_ms
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time",
        current_task().unwrap().pid.0
    );
    let ms = get_time_ms();
    let tmp_timeval = TimeVal{sec: ms / 1_000, usec: (ms * 1_000) % 1_000_000};
    let mut remain_len = core::mem::size_of::<TimeVal>();
    let buffers = translated_byte_buffer(current_user_token(), _ts as *const u8, remain_len);
    let mut tmp_timeval_ptr = &tmp_timeval as *const TimeVal as *const u8;
    for buffer in buffers {
        // if buffer.len() <= remain_len {
            for byte in buffer {
                unsafe {
                    *byte = *tmp_timeval_ptr;
                    tmp_timeval_ptr = tmp_timeval_ptr.add(1);
                }
                remain_len -= 1;
            }
        // }
    }
    if remain_len == 0 {
        0
    } else {
        -1
    }
}
/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info",
        current_task().unwrap().pid.0
    );
    let start_running_time = get_current_start_running_time();
    let mut remain_len = core::mem::size_of::<TaskInfo>();
    println!("Sizeof TaskInfo: {}", remain_len);
    let buffers = translated_byte_buffer(current_user_token(), _ti as *const u8, remain_len);
    let mut tmp_taskinfo = TaskInfo{status: current_task().unwrap().inner_exclusive_access().task_status, syscall_times: [0; MAX_SYSCALL_NUM], time: 0};
    write_current_syscall_times_array(&mut tmp_taskinfo.syscall_times);
    let mut tmp_taskinfo_ptr = &tmp_taskinfo as *const TaskInfo as *const u8;
    let current_time = get_time_ms();
    tmp_taskinfo.time = current_time - start_running_time;
    println!("DEBUG: sys_task_info: info.isRunning{}, info.time{}, current_time{}, start_running_time{}", tmp_taskinfo.status==TaskStatus::Running, tmp_taskinfo.time, current_time, start_running_time);
    for buffer in buffers {
        // if buffer.len() <= remain_len {
        //print!("Buffer-----DEBUG------");
            for byte in buffer {
                unsafe {
                    *byte = *tmp_taskinfo_ptr;
                    //print!("{}", *byte);
                    tmp_taskinfo_ptr = tmp_taskinfo_ptr.add(1);
                }
                remain_len -= 1;
            }
        // }
    }
    /*println!("remain:{}\n__________DEBUG___________", remain_len);
    unsafe {tmp_taskinfo_ptr = tmp_taskinfo_ptr.sub(core::mem::size_of::<TaskInfo>());}

    for idx in 0..core::mem::size_of::<TaskInfo>() {
        unsafe {
            print!("{}", *tmp_taskinfo_ptr.add(idx));
        }
    }*/
    // unsafe {
    //     (*converted).status = get_current_status();
    //     write_syscall_times_array(&mut (*converted).syscall_times);
    //     (*converted).time = get_time_ms() - get_start_running_time();
    // }
    if remain_len == 0 {
        0
    } else {
        -1
    }
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap",
        current_task().unwrap().pid.0
    );
    if VirtAddr::from(_start).aligned() && (_port & (!0x7usize) == 0) && (_port & 0x7usize != 0) {
        let current = current_task().unwrap();
        let mut inner = current.inner_exclusive_access();
        let mut perm = MapPermission::U;
        if _port & 0x1 != 0 {
            perm |= MapPermission::R;
        }
        if _port & 0x2 != 0 {
            perm |= MapPermission::W;
        }
        if _port & 0x4 != 0 {
            perm |= MapPermission::X;
        }
        let mut current_addr: usize = _start;
        while current_addr < _start + _len {
            match inner.memory_set.translate(VirtAddr(current_addr).floor()) {
                Some(pte) => {
                    if pte.is_valid() {
                        println!("DEBUG: PTE found in PageTable: {}", pte.bits);
                        return -1;
                    }
                },
                None => {
                }
            }
            current_addr += PAGE_SIZE;
        }

        inner.memory_set.insert_framed_area(VirtAddr::from(_start).floor().into(), VirtAddr(_start + _len).ceil().into(), perm);
    
        println!("DEBUG: sys_mmap returns 0, _start={}", _start);
        return 0;
    }
    println!("DEBUG: sys_mmap returns -1, _start={}", _start);
    -1
}


/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap",
        current_task().unwrap().pid.0
    );
    if VirtAddr::from(_start).aligned() {
        let current = current_task().unwrap();
        let mut inner = current.inner_exclusive_access();
        // Verify whether [start, start+len) has been mapped. If not, return -1
        let mut current_addr: usize = _start;
        while VirtAddr(current_addr).floor() < VirtAddr(_start + _len).ceil() {
            // test if there is a PTE corresponding to current_addr
            match inner.memory_set.translate(VirtAddr(current_addr).floor()) {
                Some(pte) => {
                    if !pte.is_valid() {
                        println!("DEBUG: munmap: PTE invalid in PageTable: {}", pte.bits);
                        return -1;
                    }
                },
                None => {
                    println!("DEBUG: munmap: no PTE in PageTable");
                    return -1;
                }
            }
            current_addr += PAGE_SIZE;
        }
        inner.memory_set.remove_area_with_start_vpn(VirtAddr::from(_start).floor());
        println!("DEBUG: sys_munmap returns 0, _start={}", _start);
        return 0;
    }
    println!("DEBUG: sys_munmap returns -1, _start={}", _start);
    -1
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}
