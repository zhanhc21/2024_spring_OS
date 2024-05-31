use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;

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
    let ret: isize;
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        ret = id as isize;
    } else {
        process_inner.mutex_list.push(mutex);
        ret = process_inner.mutex_list.len() as isize - 1;
    }

    // update mutex_available
    let mutex_available = &mut process_inner.mutex_available;
    while mutex_available.len() < (ret + 1) as usize {
        mutex_available.push(0);
    }
    mutex_available[ret as usize] = 1;

    drop(process_inner);
    ret
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

    // update mutex_need / mutex_allocation
    let cur_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    for tid in 0..process_inner.tasks.len() {
        while process_inner.mutex_need[tid].len() < (mutex_id + 1) {
            process_inner.mutex_need[tid].push(0);
            process_inner.mutex_allocation[tid].push(0);
        }
        // 请求加锁, 需求+1
        if cur_tid == tid {
            process_inner.mutex_need[tid][mutex_id] += 1;
        }
    }

    // deadlock detect
    if process_inner.deadlock_detect {
        let mut work = process_inner.mutex_available.clone();
        let mut finish = vec![false; process_inner.tasks.len()];

        // 是否找到符合条件的线程
        let mut is_found = false;
        for tid in 0..process_inner.tasks.len() {
            let need = &process_inner.mutex_need;
            if finish[tid] == false && need[tid][mutex_id] <= work[mutex_id] {
                let allocation = &process_inner.mutex_allocation;
                work[mutex_id] += allocation[tid][mutex_id];
                finish[tid] = true;
                is_found = true;
            }
        }
        if !is_found {
            // 出现死锁
            if !finish.iter().all(|&value| value) {
                return -0xDEAD;
            }
        }
    }

    process_inner.mutex_available[mutex_id] -= 1;
    process_inner.mutex_allocation[cur_tid][mutex_id] += 1;
    process_inner.mutex_need[cur_tid][mutex_id] -= 1;

    drop(process_inner);
    drop(process);
    mutex.lock();
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

    let cur_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.mutex_available[mutex_id] += 1;
    process_inner.mutex_allocation[cur_tid][mutex_id] -= 1;

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
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };

    // update semaphore_available
    let available = &mut process_inner.semaphore_available;
    while available.len() < (id + 1) {
        available.push(0);
    }
    available[id] = res_count;

    // init need / allocation
    for tid in 0..process_inner.tasks.len() {
        while process_inner.semaphore_allocation[tid].len() < (id + 1) {
            process_inner.semaphore_allocation[tid].push(0);
        }
        while process_inner.semaphore_need[tid].len() < (id + 1) {
            process_inner.semaphore_need[tid].push(0);
        }
    }

    drop(process_inner);
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
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    let cur_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.semaphore_available[sem_id] += 1;
    while process_inner.semaphore_allocation[cur_tid].len() < sem_id + 1 {
        process_inner.semaphore_allocation[cur_tid].push(0);
    }
    process_inner.semaphore_allocation[cur_tid][sem_id] -= 1;

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

    let cur_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    while process_inner.semaphore_need[cur_tid].len() < (sem_id + 1) {
        process_inner.semaphore_need[cur_tid].push(0);
    }
    // 请求加锁, 需求+1
    process_inner.semaphore_need[cur_tid][sem_id] += 1;

    // deadlock detect
    if process_inner.deadlock_detect {
        let mut work = process_inner.semaphore_available.clone();
        let mut finish = vec![false; process_inner.tasks.len()];

        // // 是否找到符合条件的线程
        // loop {
        //     let mut is_found = false;
        //     for tid in 0..process_inner.tasks.len() {
        //         // 遍历所有资源
        //         let mut valid = true;
        //         for res in work.iter().enumerate() {
        //             while process_inner.semaphore_need[tid].len() < (res.0 + 1) {
        //                 process_inner.semaphore_need[tid].push(0);
        //             }
        //             if process_inner.semaphore_need[tid][res.0] > *res.1 {
        //                 valid = false;
        //             }
        //         }
        //
        //         if !finish[tid] && valid {
        //             while process_inner.semaphore_allocation[tid].len() < (sem_id + 1) {
        //                 process_inner.semaphore_allocation[tid].push(0);
        //             }
        //             let allocation = &process_inner.semaphore_allocation;
        //             assert_ne!(allocation[tid].len(), 0);
        //
        //             work[sem_id] += allocation[tid][sem_id];
        //             finish[tid] = true;
        //             is_found = true;
        //         }
        //     }
        //     if !is_found {
        //         break;
        //     }
        // }
        // // 出现死锁
        // if !finish.iter().all(|&value| value) {
        //     return -0xDEAD;
        // }

        // 是否找到符合条件的线程
        loop {
            let mut is_found = false;
            for tid in 0..process_inner.tasks.len() {
                // 遍历所有资源
                // If any semaphore's need exceeds the remaining, 'can_proceed' will be false
                let can_proceed = !work.iter().enumerate().any(|(sem_id, &sem_remain)| {
                    while process_inner.semaphore_need[tid].len() < (sem_id + 1) {
                        process_inner.semaphore_need[tid].push(0);
                    }
                    process_inner.semaphore_need[tid][sem_id] > sem_remain
                });


                if !finish[tid] && can_proceed {
                    finish[tid] = true;
                    work.iter_mut().enumerate().for_each(|(pos, ptr)| {
                        while process_inner.semaphore_allocation[tid].len() < (pos + 1) {
                            process_inner.semaphore_allocation[tid].push(0);
                        }
                        *ptr += process_inner.semaphore_allocation[tid][pos];
                    });
                    is_found = true;
                }
            }
            if !is_found {
                break;
            }
        }
        // 出现死锁
        if !finish.iter().all(|&value| value) {
            return -0xDEAD;
        }
    }

    // assert_ne!(process_inner.semaphore_available[sem_id], 0);
    if process_inner.semaphore_available[sem_id] >= 1 {
        process_inner.semaphore_available[sem_id] -= 1;
        while process_inner.semaphore_allocation[cur_tid].len() < (sem_id + 1) {
            process_inner.semaphore_allocation[cur_tid].push(0);
        }
        process_inner.semaphore_allocation[cur_tid][sem_id] += 1;
        process_inner.semaphore_need[cur_tid][sem_id] -= 1;
    }

    drop(process_inner);
    sem.down();
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
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if _enabled == 1 {
        process_inner.deadlock_detect = true;
    } else if _enabled == 0 {
        process_inner.deadlock_detect = false;
    } else {
        return -1;
    }
    drop(process_inner);
    0
}
