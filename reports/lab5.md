1. 从 pid2process 中移除，将子进程交给 init，从任务管理器移除任务，回收页表，清除文件描述符。其他线程的 TaskControlBlock 可能在 ready_queue 或 processor.current 中、ProcessControlBlockInner::tasks 中。我认为前两者里的不需要被回收，因为 take_current_task 函数会把 processor.current 的所有权移走，执行该线程时 ready_queue 肯定没有 TaskControlBlock。

2. Mutex2 似乎是希望省去 locked = false -> locked = true 两个赋值，这样在其 lock 函数中可能仍以为 mutex 还被锁定，无法进入临界区。（不知道我理解的对不对）