//! Implementation of [`TaskManager`]
//!
//! It is only used to manage processes and schedule process based on ready queue.
//! Other CPU process monitoring functions are in Processor.

use super::{ProcessControlBlock, TaskControlBlock, TaskStatus, get_processor_id};
use crate::once_cell::race::OnceBox;
use crate::sync::SMPSafeCell;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
    
    /// The stopping task, leave a reference so that the kernel stack will not be recycled when switching tasks
    stop_task: Option<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            stop_task: None,
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        if self.ready_queue.is_empty() {
            return None;
        }
        let mut min_stride_block: usize = 0;
        let min_stride = self.ready_queue.front_mut().unwrap().inner_exclusive_access().stride;
        let mut index: usize = 0;
        for block in self.ready_queue.iter() {
            if block.inner_exclusive_access().stride < min_stride {
                min_stride_block = index;
            }
            index += 1;
        }
        self.ready_queue.remove(min_stride_block)
        //self.ready_queue.pop_front()
    }
    pub fn remove(&mut self, task: Arc<TaskControlBlock>) {
        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| Arc::as_ptr(t) == Arc::as_ptr(&task))
        {
            self.ready_queue.remove(id);
        }
    }
    /// Add a task to stopping task
    pub fn add_stop(&mut self, task: Arc<TaskControlBlock>) {
        // NOTE: as the last stopping task has completely stopped (not
        // using kernel stack any more, at least in the single-core
        // case) so that we can simply replace it;
        self.stop_task = Some(task);
    }

}

/// TASK_MANAGER instance
pub static TASK_MANAGER: OnceBox<Vec<SMPSafeCell<TaskManager>>> = OnceBox::new();
/// PID2PCB instance (map of pid to pcb)
pub static PID2PCB: OnceBox<SMPSafeCell<BTreeMap<usize, Arc<ProcessControlBlock>>>> = OnceBox::new();

/// Add a task to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    let mut min_hart_id = 0;
    let mut min_length = core::i32::MAX as usize;
    for (hart_id, value) in TASK_MANAGER.get().unwrap().iter().enumerate() {
        let current_len = value.exclusive_access().get().ready_queue.len();
        if current_len < min_length {
            min_hart_id = hart_id;
            min_length = current_len;
        }
    }
    TASK_MANAGER.get().unwrap()[min_hart_id].exclusive_access().get_mut().add(task);
    // println!("Add task done");
}

/// Add a task to the ready queue at this
pub fn add_task_at_this_hart(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task_at_this_hart");
    TASK_MANAGER.get().unwrap()[get_processor_id()].exclusive_access().get_mut().add(task);
}

/// Wake up a task
pub fn wakeup_task(task: Arc<TaskControlBlock>) {
    trace!("kernel: TaskManager::wakeup_task");
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    add_task(task);
}

/// Remove a task from the ready queue
pub fn remove_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::remove_task");
    TASK_MANAGER.get().unwrap()[get_processor_id()].exclusive_access().get_mut().remove(task);
}

/// Fetch a task out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.get().unwrap()[get_processor_id()].exclusive_access().get_mut().fetch()
}

/// Set a task to stop-wait status, waiting for its kernel stack out of use.
pub fn add_stopping_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.get().unwrap()[get_processor_id()].exclusive_access().get_mut().add_stop(task);
}

/// Get process by pid
pub fn pid2process(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    let map = PID2PCB.get().unwrap().exclusive_access().get_mut();
    map.get(&pid).map(Arc::clone)
}

/// Insert item(pid, pcb) into PID2PCB map (called by do_fork AND ProcessControlBlock::new)
pub fn insert_into_pid2process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2PCB.get().unwrap().exclusive_access().get_mut().insert(pid, process);
}

/// Remove item(pid, _some_pcb) from PDI2PCB map (called by exit_current_and_run_next)
pub fn remove_from_pid2process(pid: usize) {
    let mut map = PID2PCB.get().unwrap().exclusive_access().get_mut();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}
