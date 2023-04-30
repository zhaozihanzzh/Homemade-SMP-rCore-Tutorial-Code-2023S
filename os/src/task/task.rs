//! Types related to task management

use super::TaskContext;
use alloc::collections::BTreeMap;

/// The task control block (TCB) of a task.
//#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The task syscall counts
    pub syscall_count: BTreeMap<usize, usize>,
    /// Start running time
    pub start_time: usize,
}

//impl Copy for TaskControlBlock {}
/* impl Clone for TaskControlBlock {
    fn clone(&self) -> Self {
        let cloned = TaskControlBlock {task_status: self.task_status.clone(), task_cx: self.task_cx.clone(), syscall_count: self.syscall_count.clone(), start_time: self.start_time};
        cloned
    }
} */

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
