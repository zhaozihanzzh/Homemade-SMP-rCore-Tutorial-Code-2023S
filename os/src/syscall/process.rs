//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, get_current_status, get_start_running_time, write_syscall_times_array, current_user_token, task_mmap, task_munmap,
    },
    mm::{translated_byte_buffer, VirtAddr},
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
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let ms = get_time_ms(); // get_time_us causes precision loss
    // See https://github.com/LearningOS/rCore-Tutorial-Code-2022S/pull/4
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
    let start_running_time = get_start_running_time();
    trace!("kernel: sys_task_info");
    let mut remain_len = core::mem::size_of::<TaskInfo>();
    println!("Sizeof TaskInfo: {}", remain_len);
    let buffers = translated_byte_buffer(current_user_token(), _ti as *const u8, remain_len);
    let mut tmp_taskinfo = TaskInfo{status: get_current_status(), syscall_times: [0; MAX_SYSCALL_NUM], time: 0};
    write_syscall_times_array(&mut tmp_taskinfo.syscall_times);
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

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap");
    if VirtAddr::from(_start).aligned() && (_port & (!0x7usize) == 0) && (_port & 0x7usize != 0) {
        let ret = task_mmap(VirtAddr(_start), _len, _port);
        println!("DEBUG: sys_mmap returns {}, _start={}", ret, _start);
        return ret;
    }
    println!("DEBUG: sys_mmap returns -1, _start={}", _start);
    -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if VirtAddr::from(_start).aligned() {
        let ret = task_munmap(VirtAddr(_start), _len);
        println!("DEBUG: sys_munmap returns {}, _start={}", ret, _start);
        return ret;
    }
    println!("DEBUG: sys_munmap returns -1, _start={}", _start);
    -1
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
