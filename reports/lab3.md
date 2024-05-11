## Ch5

### 占海川 2021050009

----

#### 实现功能

1) 仿照`sys_exec()`函数, 通过path解析出elf文件; 仿照初始进程的创建过程, 调用TaskControlBlock::new(elf)创建子进程. 设置好
   子进程的parent, 将其加入到ready_queue中. 最后将child加入父进程的children
2) 在`TaskControlBlockInner`中新增`stride`与`priority`字段, 将`BIG_STRIDE`设置为0x10000000. 完成`sys_set_priority()`函数.
   更改`TaskManager::fetch()`的实现, 在每次选择进程时遍历`ready_queue`,
   并选择stride最小的进程, 记录其下标, 令stride加上`pass = BIG_STRIDE / priority`, 最后调用remove(index)返回进程.

----

#### 简答题

stride 算法原理非常简单，但是有一个比较大的问题。例如两个 pass = 10 的进程，使用 8bit 无符号整形储存 stride， p1.stride =
255, p2.stride = 250，在 p2 执行一个时间片后，理论上下一次应该 p1 执行。

- 实际情况是轮到 p1 执行吗？为什么?
  实际上会执行p2. 因为发生了溢出, 此时p2.stride=4, 会被优先执行

我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在不考虑溢出的情况下 , 在进程优先级全部 >= 2
的情况下，如果严格按照算法执行，那么 STRIDE_MAX – STRIDE_MIN <= BigStride / 2。

- 为什么? 尝试简单说明（不要求严格证明）。
  stride += BigStride / priority, 每个进程的priority都大于等于2, 因此所有步长小于等于BigStride / 2, 步长相差不大.
  且步长小的进程调度次数多, 步长大的进程调度次数少, 从而确保最大步长差不会超过BigStride / 2

  - 已知以上结论，考虑溢出的情况下，可以为 Stride 设计特别的比较器，让 BinaryHeap<Stride> 的 pop 方法能返回真正最小的
    Stride。补全下列代码中的 partial_cmp 函数，假设两个 Stride 永远不会相等。TIPS: 使用 8 bits 存储 stride, BigStride = 255,
    则: (125 < 255) == false, (129 < 255) == true.
    ```
    use core::cmp::Ordering;

    struct Stride(u64);
  
    impl PartialOrd for Stride {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            let diff = self.0.wrapping_sub(other.0);
            if diff <= BigStride / 2 {
                Some(Ordering::Less)
            } else if diff > BigStride / 2 {
                Some(Ordering::Greater)
            } else {
                Some(Ordering::Equal)
            }
        }
    }
  
    impl PartialEq for Stride {
        fn eq(&self, other: &Self) -> bool {
            false
        }
    }
    ```

----

#### honor code

本人独立完成本次实验, 未抄袭他人代码, 也未将代码提供给其他人或上传公开仓库