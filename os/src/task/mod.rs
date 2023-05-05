//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;


use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE};
use crate::loader::{get_app_data, get_num_app};
use crate::mm::{MapPermission, VirtAddr};
use crate::sync::UPSafeCell;
use crate::timer::get_time_ms;
use crate::trap::TrapContext;
use alloc::vec::Vec;

use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        if next_task.is_started {
            println!("ERROR: HAS STARTED!!!!!!!!!!!!!!!!!!!");
        }
        next_task.start_time = get_time_ms(); 
        if next_task.start_time == 0 {
            println!("ERROR: start_time is still 0 after assignment!");
        }
        next_task.is_started = true;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            if inner.tasks[next].start_time == 0 {
                if inner.tasks[next].is_started {
                    println!("ERROR: HAS STARTED!!!!!!!!!!!!!!!!!!!");
                }
                inner.tasks[next].start_time = get_time_ms();
                inner.tasks[next].is_started = true;
            } else if !inner.tasks[next].is_started {
                println!("ERROR: WRONG START TIME!");
            } 
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
    /// Get the status of current task
    fn get_current_status(&self) -> TaskStatus {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status
    }
    /// Record syscall(count)
    fn record_syscall(&self, call_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_count.entry(call_id).and_modify(| call_times | *call_times += 1).or_insert(1);
    }

    /// Write syscall times to an array
    fn write_syscall_times_array(&self, syscall_times: &mut [u32; MAX_SYSCALL_NUM]) {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        for syscall_id in 0..MAX_SYSCALL_NUM {
            match inner.tasks[current].syscall_count.get(&syscall_id) {
                Some(syscall_time) => syscall_times[syscall_id] = *syscall_time as u32,
                None => syscall_times[syscall_id] = 0,
            };
        }
        
    }

    /// Get start running time
    fn get_start_running_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        println!("DEBUG: start_time={}", inner.tasks[current].start_time);
        inner.tasks[current].start_time
    }

    /// Map Memory
    fn mmap(&self, start: VirtAddr, len: usize, port: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let mut perm = MapPermission::U;
        if port & 0x1 != 0 {
            perm |= MapPermission::R;
        }
        if port & 0x2 != 0 {
            perm |= MapPermission::W;
        }
        if port & 0x4 != 0 {
            perm |= MapPermission::X;
        }
        let mut current_addr: usize = start.into();
        while current_addr < usize::from(start) + len {
            match inner.tasks[current].memory_set.translate(VirtAddr(current_addr).floor()) {
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

        inner.tasks[current].memory_set.insert_framed_area(start.floor().into(), VirtAddr(usize::from(start) + len).ceil().into(), perm);
        0
    }

    /// Ummap Memory
    fn munmap(&self, start: VirtAddr, len: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // Verify whether [start, start+len) has been mapped. If not, return -1
        let mut current_addr: usize = start.into();
        while VirtAddr(current_addr).floor() < VirtAddr(usize::from(start) + len).ceil() {
            // test if there is a PTE corresponding to current_addr
            match inner.tasks[current].memory_set.translate(VirtAddr(current_addr).floor()) {
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
        inner.tasks[current].memory_set.remove_framed_area(VirtAddr::from(start), VirtAddr::from(usize::from(start) + len));
        0
    }
}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}

/// Get current status
pub fn get_current_status() -> TaskStatus {
    TASK_MANAGER.get_current_status()
}

/// Record syscall
pub fn record_syscall(id: usize) {
    TASK_MANAGER.record_syscall(id);
}

/// Write syscall times to an array
pub fn write_syscall_times_array(syscall_times: &mut [u32; MAX_SYSCALL_NUM]) {
    TASK_MANAGER.write_syscall_times_array(syscall_times);
}

/// Get start running time
pub fn get_start_running_time() -> usize {
    TASK_MANAGER.get_start_running_time()
}

/// mmap
pub fn task_mmap(start: VirtAddr, len: usize, port: usize) -> isize {
    TASK_MANAGER.mmap(start, len, port)
}

/// munmap
pub fn task_munmap(start: VirtAddr, len: usize) -> isize {
    TASK_MANAGER.munmap(start, len)
}