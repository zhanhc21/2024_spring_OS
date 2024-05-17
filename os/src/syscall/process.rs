//! Process management syscalls
//!
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{
    config::MAX_SYSCALL_NUM,
    fs::{open_file, OpenFlags, File},
    mm::{translated_refmut, translated_str, VirtAddr, PhysAddr, PageTable},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, TaskControlBlock,
        suspend_current_and_run_next, TaskStatus, get_start_time, get_syscall_times, mmap, munmap,
        TaskControlBlockInner
    },
    timer::{get_time_us, get_time_ms},
};
use crate::config::TRAP_CONTEXT_BASE;
use crate::mm::{KERNEL_SPACE, MemorySet};
use crate::sync::UPSafeCell;
use crate::task::{kstack_alloc, pid_alloc, TaskContext};
use crate::trap::{trap_handler, TrapContext};


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in its life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
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
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process, but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
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

/// transform addr from virtual to physical
pub fn vir_to_phy(virtual_addr: VirtAddr) -> PhysAddr {
    let vpn = virtual_addr.floor();
    let vpo = virtual_addr.page_offset();
    let page_table = PageTable::from_token(current_user_token());
    let ppn = page_table.translate(vpn).unwrap().ppn();
    let mut pa = PhysAddr::from(ppn);
    pa.0 += vpo;
    pa
}

/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is split by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
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
/// HINT: What if [`TaskInfo`] is split by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
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
    trace!(
        "kernel:pid[{}] sys_mmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let virtual_addr = VirtAddr(_start);
    // 对齐 / 其余位为0 / 有意义内存
    if !virtual_addr.aligned() || (_port & !0x7 != 0) || (_port & 0x7 == 0) {
        return -1;
    }
    mmap(_start, _len, _port)
}

/// munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let virtual_addr = VirtAddr(_start);
    // 未对齐
    if !virtual_addr.aligned() {
        return -1;
    }
    munmap(_start, _len)
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

/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    // trace!(
    //     "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
    //     current_task().unwrap().pid.0
    // );
    // let token = current_user_token();
    // let path = translated_str(token, _path);
    //
    // if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
    //     let parent = current_task().unwrap();
    //     let mut parent_inner = parent.inner_exclusive_access();
    //
    //     let child = Arc::new({
    //         let all_data = app_inode.read_all();
    //         TaskControlBlock::new(all_data.as_slice())
    //     });
    //
    //     let mut child_inner = child.inner_exclusive_access();
    //
    //     // copy fd table
    //     let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
    //     for fd in parent_inner.fd_table.iter() {
    //         if let Some(file) = fd {
    //             new_fd_table.push(Some(file.clone()));
    //         } else {
    //             new_fd_table.push(None);
    //         }
    //     }
    //
    //     // 更新 base_size, parent, fd_table, heap, brk
    //     child_inner.base_size = parent_inner.base_size;
    //     child_inner.parent = Some(Arc::downgrade(&parent));
    //     child_inner.fd_table = new_fd_table;
    //     child_inner.heap_bottom = parent_inner.heap_bottom;
    //     child_inner.program_brk = parent_inner.program_brk;
    //
    //     parent_inner.children.push(child.clone());
    //     add_task(child.clone());
    //
    //     drop(child_inner);
    //     drop(parent_inner);
    //
    //     child.getpid() as isize
    // } else {
    //     -1
    // }
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, _path);

    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let parent = current_task().unwrap();
        let mut parent_inner = parent.inner_exclusive_access();

        let all_data = app_inode.read_all();
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(all_data.as_slice());
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(&parent)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    start_time: 0,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    stride: 0,
                    priority: 16,
                })
            },
        });
        /// add child
        parent_inner.children.push(task_control_block.clone());
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        let pid = task_control_block.pid.0 as isize;
        add_task(task_control_block);

        drop(parent_inner);
        task_control_block.getpid() as isize
    } else {
        -1
    }
}

/// Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio < 2 {
        return -1;
    }
    current_task().unwrap().inner_exclusive_access().priority = _prio;
    _prio
}