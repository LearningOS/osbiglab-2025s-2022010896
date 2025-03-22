### 第六周工作总结

左晨阳 2022010896

#### 本周工作

### 学习笔记

#### nimbos 的 syscall 转发

**hello**

- hello 程序运行在 Linux 普通域中，用于监听并处理转发自 nimbos app 的 syscall 请求。在 `nimbos_setup_syscall` 中，打开 `/dev/nimbos` 并创建内存映射，用作与 nimbos 通信的管道。hello 将访问其中的 syscall 环形缓冲区，在接收到 `NIMBOS_SYSCALL_SIG` 时，读取并处理 syscall 请求。
- 环形队列的信息由 `syscall_queue_buffer_metadata` 描述，其中包含一个魔法数字用于验证通信，一个锁保证互斥访问，队列容量，以及请求指针和响应指针。nimbos 在 req_ring 中写入请求的下标，并更新 req_index，hello 自身维护已经处理的下标位置 req_index_last。类似地，hello 将处理完的响应下标写入 rsp_ring，并更新 meta 中的 rsp_index。
- syscall 请求由 `scf_descriptor` 结构描述，其中包含有效标记（未被 hello 使用）、操作码、参数地址下标和返回值。
- nimbos-driver 启动时，会通过 `misc_register` 向内核注册一个 miscdevice，并设置其操作函数。`nimbos_ioctl` 被注册为操作函数，目前只能处理 `NIMBOS_SYSCALL_SETUP` 指令。在缓冲区初始化完成后，hello 会向 nimbos-driver 发送一个 `NIMBOS_SETUP_SYSCALL` 控制命令，指示其将自身注册为 syscall 处理器。hello 进程会始终驻留而不终止。

**nimbos**

- 在 nimbos 编译时，如果使用 `RVM=on` 选项，则会使用scf模块下的转发版本 `sys_write` 和 `sys_read` 替代原始处理函数。远程调用时，nimbos 会查找未被占用的 `ScfDescriptor`，将调用信息写入对应位置，然后将下标写入 `req_ring`。如果环形队列已满，调用将会被阻塞，内核会切出该任务。
- 请求被成功写入缓冲区后，`notify` 函数会通过 `send_ipi` 发送 `SYSCALL_IPI_IRQ_NUM` 处理器间中断，这一过程使用了 APIC。hello 能够通过 Linux 内核被通知到，从而处理请求。
- 当请求被成功发送的同时，一个系统调用对应的状态变量会被加入到 `SyscallQueueBuffer.tokens` 中，用于指示该请求是否已被处理。`sys_write` 和 `sys_read` 会调用状态变量的 `wait()` 函数，等待请求被处理。nimbos 会定时运行 `handle_irq`，检查是否已经有远程调用完成，如果有则调用状态变量的 `signal()`，将返回值传递给处理函数并通知其继续运行。