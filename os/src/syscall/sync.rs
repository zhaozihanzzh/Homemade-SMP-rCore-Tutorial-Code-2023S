use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, TaskStatus};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.mutex_available[id] = 1;
        for need in process_inner.mutex_need.iter_mut() {
            need[id] = 0;
        }
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_available.push(1);
        for need in process_inner.mutex_need.iter_mut() {
            need.push(0);
        }
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    process_inner.mutex_need[current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid][mutex_id] += 1;
    if process_inner.is_detect_deadlock_enable {
        let mut is_deadlocked = true;
        for (mutex_index, available) in process_inner.mutex_available.iter().enumerate() {
            for task_need in process_inner.mutex_need.iter() {
                if task_need[mutex_index] <= *available {
                    is_deadlocked = false;
                    break;
                }
            }
            if !is_deadlocked {
                break;
            }
        }
        if is_deadlocked {
            process_inner.mutex_need[current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid][mutex_id] -= 1;
            return -0xDEAD;
        }
    }
    drop(process_inner);
    drop(process);
    mutex.lock();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.mutex_need[current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid][mutex_id] -= 1;
    process_inner.mutex_available[mutex_id] -= 1;
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    process_inner.mutex_available[mutex_id] += 1;
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.semaphore_available[id] = res_count;
        for need in process_inner.semaphore_need.iter_mut() {
            need[id] = 0;
        }
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_available.push(res_count);
        for need in process_inner.semaphore_need.iter_mut() {
            need.push(0);
        }
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    println!("Task {} called semaphore_up on sem {}", current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid, sem_id);
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    println!("semaphore_available before up: {}", process_inner.semaphore_available[sem_id]);
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    process_inner.semaphore_available[sem_id] += 1;
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let task_id = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    println!("Task {} called semaphore_down on sem {}, before that, process_inner.semaphore_need[task_id][sem_id]={} ", task_id, sem_id, process_inner.semaphore_need[task_id][sem_id]);
    process_inner.semaphore_need[task_id][sem_id] += 1;
    if process_inner.is_detect_deadlock_enable {
        let mut is_deadlocked = true;
        for (task_index, task_need) in process_inner.semaphore_need.iter().enumerate() {
            let mut is_current_task_all_available = true;
            if process_inner.tasks[task_index].is_none() || process_inner.tasks[task_index].as_ref().unwrap().inner_exclusive_access().task_status != TaskStatus::Running {
                if process_inner.tasks[task_index].is_none() {
                    println!("In task {}: Jumped to next task.", task_id);
                    continue;
                }
            }
            for (sem_index, available) in process_inner.semaphore_available.iter().enumerate() {
                println!("In task {}: Iter task_index = {}, task_need[sem_index={}]={}, available={}", task_id, task_index, sem_index, task_need[sem_index], available);
                if task_need[sem_index] > *available {
                    is_current_task_all_available = false;
                    println!("In task {}: Task {} is locked.", task_id, task_index);
                    break;
                }
            }
            if is_current_task_all_available {
                println!("In task {}: Task {} is not deadlocked.", task_id, task_index);
                is_deadlocked = false;
                break;
            }
        }
        if is_deadlocked {
            process_inner.semaphore_need[task_id][sem_id] -= 1;
            println!("*_* *_* Deadlock detected!");
            return -0xDEAD;
        }
    }
    drop(process_inner);
    sem.down();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.semaphore_need[task_id][sem_id] -= 1;
    process_inner.semaphore_available[sem_id] -= 1;
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    if enabled == 0 {
        current_process().inner_exclusive_access().is_detect_deadlock_enable = false;
        0
    } else if enabled == 1 {
        current_process().inner_exclusive_access().is_detect_deadlock_enable = true;
        0
    } else {
        -1
    }
}
