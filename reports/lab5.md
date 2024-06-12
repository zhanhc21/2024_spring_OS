## Ch8

### 占海川 2021050009

----

#### 实现功能

1) 向进程管理块中新增成员变量`deadlock_detect`, `<mutex/semaphore>_available`, `<mutex/semaphore>_allocation`, `<mutex/semaphore>_need`
分别用于记录是否开启死锁检测, 以及互斥锁/信号量的可用/分配/需求情况, 后面三个数组在创建新进程与调用对应的creat函数时进行初始化
2) 完成`sys_enable_deadlock_detect`函数, 用于开启死锁检测
3) 修改`sys_mutex_create`与`sys_semaphore_create`, 在创建锁更新对应线程的可用资源/分配/需求
4) 修改`sys_mutex_lock`与`sys_semaphore_down`, 依据算法, 遍历所有资源, 更新以上三个数组, 并检查是否存在死锁
5) 修改`sys_mutex_unlock`与`sys_semaphore_up`, 将available数组对应线程的可用资源+1, 分配-1

----

#### 简答题
1. 在我们的多线程实现中，当主线程 (即 0 号线程) 退出时，视为整个进程退出， 此时需要结束该进程管理的所有线程并回收其资源。 
   - 需要回收的资源有哪些？ - 其他线程的 TaskControlBlock 可能在哪些位置被引用，分别是否需要回收，为什么？
   需要回收的资源为TaskUserRes中的trap_context/user_stack/thread_id, 文件描述符, 线程池, memory_set, children; 
   其他线程的控制块可能在被锁/信号量/条件变量阻塞时被引用(即ready_queue与timer), 这部分需要手动回收; 在线程池中被引用, 这部分需要手动回收; 
   可能在锁/信号量/条件变量中被引用, 这部分不需要单独回收, 因为解锁后引用计数归零自动析构, 且由于`TaskUserRes`已经被回收, 不会产生资源的二次析构问题

2. 对比以下两种 Mutex.unlock 的实现，二者有什么区别？这些区别可能会导致什么问题？

```
impl Mutex for Mutex1 {
    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        mutex_inner.locked = false;
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            add_task(waking_task);
        }
    }
}

impl Mutex for Mutex2 {
    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            add_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
```
实现二只有在没有线程被阻塞的情况下才会释放锁，当存在线程被阻塞时，锁会转交所有权给下一线程。而实现一在判断是否需要唤醒阻塞线程前会释放锁，可能导致其他线程
抢先获取锁， 出现重复借用错误

----

#### honor code

本人独立完成本次实验, 未抄袭他人代码, 也未将代码提供给其他人或上传公开仓库