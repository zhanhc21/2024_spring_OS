## Ch6

### 占海川 2021050009

----

#### 实现功能
1) 修改Ch5中`spwan`的实现, 使其拷贝父进程的文件描述符表
2) 向`DiskInode`中添加成员变量`nlink`, 用于记录文件的硬链接数, 同时令直接索引`direct`的块数-1
3) 实现`sys_fstat`, 调用`translated_refmut`函数, 将指针转换为可变引用, 并利用多态实现`fstat`: 
    - 对于OSInode, nlink与mode字段需逐层调用Inode与DiskInode中的方法; 获取inode_index的计算实现在`Inode`中, 即(block_id - start_block_id) * inode_num_per_block + block_offset/inode_size
    - 对于StdIn与StdOut, 直接返回-1即可
4) 实现`sys_linkat`, 在`Inode`类中实现`linkat`, 仿照`create`函数, 向root_inode中新增目录项, 随后将对应的`DiskInode`的`nlink`加1
5) 实现`sys_unlinkat`, 在`Inode`类中实现`unlinkat`, 遍历目录项, 若被删除的目录项为最后一项可直接将DiskInode大小减去目录项大小, 否则将最后一项拷贝至删除项处, 并将DiskInode大小减去目录项大小. 最后将对应的`DiskInode`的`nlink`减1, 若硬连接数为0, 则仿照`Clear`函数remove数据块
----

#### 简答题
##### CH6
1. 在我们的easy-fs中，root inode起着什么作用？如果root inode中的内容损坏了，会发生什么?
    - root inode是文件系统的根目录, 在我们的文件系统中, 所有的create/clear/link等操作均通过root inode进行, 且所有的文件均存放在root inode的block device下. 
    - 若root inode中的内容损坏, 则无法进行任何文件操作
##### CH7
1. 举出使用 pipe 的一个实际应用的例子
   linux shell中`|`为管道符, 可以将一个命令的输出作为另一个命令的输入, 例如`cat file | wc -l`可以统计文件行数
2. 如果需要在多个进程间互相通信，则需要为每一对进程建立一个管道，非常繁琐，请设计一个更易用的多进程通信机制
   通过若干条"总线"收发消息, 消息组织为队列形式, 先进先出. 进程可创建BUS, 且一条BUS可共享给若干进程. 进程在发送消息时可为消息添加标识符, 用于区分消息接收者

----

#### honor code

本人独立完成本次实验, 未抄袭他人代码, 也未将代码提供给其他人或上传公开仓库