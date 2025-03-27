### 第六周工作总结

左晨阳 2022010896

#### 本周工作

1. 阅读并理解了 cRTOS 中的 syscall 转发机制，包括 nimbos app 和 hello 的通信方式，以及 nimbos-driver 的注册和初始化过程；
2. 构建了支持用户态中断（uintr）的 Linux 环境，将 RVM1.5 迁移到支持用户态中断的 QEMU + Linux 内核上；

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

#### 支持用户态中断的 Linux 环境

- QEMU 并非原生支持用户态中断，选用 https://github.com/OS-F-4/usr-intr 的实现增加对 uintr 的支持。该项目为 QEMU 新增了5条指令支持，实现了指令翻译和捕捉、新的硬件状态设定，并且修改中断处理实现接收。
- Linux 内核主分支也没有实现用户态中断，intel 的 uintr-linux-kernel 仓库已经停止维护，但功能比较完整。选用 https://github.com/OS-F-4/usr-intr 中与 QEMU 验证适配的内核实现。

一开始按照 QEMU 项目的提示构建了内核和文件系统，得到最小化 Linux 环境，但是环境不完整不易通过 ssh 连接。遂尝试在 20.04 Ubuntu 中安装内核，发现 5.15 版本 uintr-linux-kernel 可以正常使用。

#### RVM1.5 迁移

遇到的问题以及解决方案：
- 在 uintr-linux-kernel 下编译 jailhouse 驱动程序不成功，tools 目录下不产生可执行文件，但是看不到任何报错。
  - 解决：向助教请教得知，不同内核 KBUILD 存在不兼容，Makefile 中 always 选项应当改为 always-y。
- jailhouse 编译不成功，提示 cpu_up 和 cpu_down 未定义，并且存在其他调用参数错误。
  - 解决：更换 jailhouse 驱动版本为 3.14，移除 KMSAN 调用，并且查询资料，手动修改 set_huge_pte_at 和 vmap_range_noflush 的调用参数，最终成功编译。
- 在普通 QEMU 中，RVM 正常启动，但是进入 linux 后报错：
    ```
    [   16.026021] invalid opcode: 0000 [#1] SMP NOPTI
    ```
    此后终端正常运行，命令可以执行，但无法正常关机
  - 解决：阅读日志发现，错误发生在 RIP: 0010:delay_halt_tpause+0xd/0x20，表明是 TPAUSE 指令执行出错。向助教请教得知，这与 VMCS 给 Linux 客户机的配置异常有关，尝试打开 USR_WAIT_PAUSE 选项，却导致 RVM 启动 Linux 异常。遂在 QEMU 启动参数中添加 `-waitpkg`，规避掉 TPAUSE 指令，成功启动。
- 在支持 uintr 的 QEMU 中，RVM 启动异常，CPU 占用猛增，日志不输出。
  - 解决：添加 println! 二分查找，发现日志输出函数产生错误，__rdtscp 无法正确获取到时间戳，并且导致阻塞。移除日志中的时间戳，成功启动 RVM。