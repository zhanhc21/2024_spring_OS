## Ch3
### 占海川 2021050009

----
#### 实现功能
  1) 在`syscall/process.rs`中完成`sys_task_info`函数, 更新`TaskInfo`并返回其可变引用
  2) 向`struct TaskControlBlock`添加参数用于记录任务起始时间与各系统调用次数
  3) 在`task/mod.rs/TaskManager::run_first_task`与`run_next_task`中更新任务起始时间; 在`syscall/mod.rs/syscall`中记录当前任务的各系统调用次数
  4) 在`TaskManager`类中新增成员函数用于增加系统调用计数/返回当前任务的系统调用次数/返回当前任务的起始时间

----
#### 简答题
  1. - RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
     ```
     [kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003ac, kernel killed it.
     [kernel] IllegalInstruction in application, kernel killed it.
     [kernel] IllegalInstruction in application, kernel killed it.
     ```  
     
  2. 1) `trap_handler`的返回值为cx, 则进入`__restore`时, `a0`代表了TrapContext的地址; 
     当初始化任务控制块时会调用`__restore`向内核栈顶压入初始上下文; 
     当完成Trap分发与处理后使用`__restore`从内核栈上的Trap上下文恢复寄存器, 并回到用户态
     2) `sstatus`: 其中包含了`*PIE, *IE, *PP`, 分别保存了Trap嵌套, 全局中断使能与发生Trap前模式, `sret`需要通过`spp`决定切换的特权状态;
     `sepc`: 保存了引起异常的指令的地址, `sret`会回到`sepc`处继续执行; 
     `sscratch`: 保存用户栈的地址, 使控制流从内核栈回到用户栈
     3) `x2`寄存器为栈指针, 其会在后面恢复为用户栈; `x4`寄存器为线程指针, 这里应用程序不需要tp来管理线程信息
     4) `sp`指向用户栈栈顶，`sscratch`指向内核栈栈顶
     5) 发生状态切换的指令为`sret`, 在执行该指令时, 处理器会从`sstatus`寄存器中恢复之前的特权态即U态; 从`sepc`读取引起异常的指令地址
     将控制流返回到该地址, 在用户态下继续执行应用程序
     6) `sp`指向内核栈栈顶，`sscratch`指向用户栈栈顶
     7) 发生状态切换的指令为ecall


----
#### honor code
  本人独立完成本次实验, 未抄袭或参考其他人代码, 也未将代码提供给其他人