//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
        current_user_token, get_start_time, get_syscall_times, mmap, munmap
    },
    timer::{
        get_time_ms, get_time_us
    },
    mm::{
        VirtAddr, PhysAddr, PageTable
    },
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

/// transform addr from virtual to physical
pub fn vir_to_phy(virtual_addr: VirtAddr) -> PhysAddr {
    let vpn = virtual_addr.floor();
    let vpo = virtual_addr.page_offset();
    let page_table = PageTable::from_token(current_user_token());
    let ppn = page_table.translate(vpn).unwrap().ppn();
    let mut pa= PhysAddr::from(ppn);
    pa.0 += vpo;
    pa
}

/// get time with second and microsecond`
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let physical_addr = vir_to_phy(VirtAddr(_ts as usize));
    let physical_ts = physical_addr.0 as *mut TimeVal;

    let us = get_time_us();
    unsafe {
        *physical_ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let physical_addr = vir_to_phy(VirtAddr(_ti as usize));
    let physical_ti = physical_addr.0 as *mut TaskInfo;

    let ms = get_time_ms();
    unsafe {
        *physical_ti = TaskInfo {
            status: TaskStatus::Running,
            syscall_times: get_syscall_times(),
            time: ms - get_start_time(),
        };
    }
    0
}

/// mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let virtual_addr = VirtAddr(_start);
    // 对齐 / 其余位为0 / 有意义内存
    if virtual_addr.aligned() && (_port & !0x7 == 0) && (_port & 0x7 != 0) {
        return mmap(_start, _len, _port)
    }
    -1
}

// TODO: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    // let virtual_addr = VirtAddr(_start);
    // if virtual_addr.aligned() {
    //     return munmap(_start, _len)
    // }
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
