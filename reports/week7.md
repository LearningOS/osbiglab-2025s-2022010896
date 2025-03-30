### 第七周工作总结

左晨阳 2022010896

#### 本周工作

#### 代码阅读报告

**arch/x86/include/asm/uintr.h**

定义了用户态中断直接相关的结构：

- uintr_upid：与 intel 手册中 UPID 定义保持一致。UPID 是用户态中断的通信枢纽，由接收者配置，包含中断是否需要通知、通知的 APIC 目标、通知向量以及待处理的中断请求位。
- uintr_upid_ctx：uintr_upid 的上下文信息，包含双向链表指针、接受方 Task 指针、引用计数、活跃标记 receiver_active、
- uintr_uitt_entry：与 intel 手册中 UITTE 保持一致，是 UITT 中的表项，描述一个中断发起行为。包含中断向量（64种取值，对应 UPID 中的 64 个请求位）、UPID 地址以及有效位。
- uintr_uitt_ctx：UITT 表结构，需要互斥访问，包含互斥锁、uintr_uitt_entry 指针、引用计数、每一个表项对应的 uintr_upid_ctx 指针以及表项占用位图 uitt_mask。

**arch/x86/kernel/uintr.c**

学习笔记：

1. 每个 task 都可以有各自的 UITT，但是也可以共享。
2. 每个 task 只能注册一个 uintr handler。

内存管理：

实现了 check_upid_ref、put_upid_ref、get_upid_ref、put_uitt_ref、check_uitt_ref、get_uitt_ref 等 UPID 和 UITT 引用计数管理函数。实际上整个文件中诸多代码都是为了正确实现引用计数的管理，下不再赘述。

alloc_upid 和 free_upid 函数实现了 UPID 的分配和释放，始终将 uintr_upid 包装在 uintr_upid_ctx 中。初始时引用计数为 1，task 指向 current，receiver_active 为 true（将始终为 true，直到 uintr_free 被调用），

相应地，alloc_uitt 和 free_uitt 实现了 UITT 的分配和释放，UITT 同样以 uintr_uitt_ctx 为管理单元，其中只需要特殊处理锁的初始化和释放。

杂项：

- do_uintr_register_vector：将某中断向量 uvec 加入到 uintr_upid_ctx.uvec_mask 中，表示当前进程准备处理该类型中断。只有已经注册有 handler 的 task 才可以注册中断向量。
- do_uintr_unregister_vector：并不会修改 uintr_upid_ctx.uvec_mask，只会降低 upid_ctx 的引用计数。

- uintr_set_sender_msrs：更新保存的 MSR 寄存器值，加入 UITT 相关配置项。具体而言，UITT 地址被写入 MSR_IA32_UINTR_TT。UITT 表项有效 mask（UINTR_MAX_UITT_NR-1）被写入 MSR_IA32_UINTR_MISC 低 32 为。

进程退出时，如下函数被调用进行清理：

- uintr_free：

系统调用：

- sys_uintr_ipi_fd：返回一个文件描述符 uipi_fd，其中包含当前进程的 uitt_ctx，可以在用户进程间传递。该文件描述符绑定到操作 uipifd_fops。uipifd_open 中，如果 mm->context.uitt_ctx 为空，将其更新为 file->private_data（系统调用创建文件描述符时，会读取 mm->context.uitt_ctx 作为 file->private_data）。uipifd_ioctl 支持的操作只有 UIPI_SET_TARGET_TABLE，即依据文件描述符 private_data 设置当前进程的 UITT，并调用 uintr_set_sender_msrs。
- sys_uintr_vector_fd：注册自身想要处理的中断向量并返回文件描述符 uvecfd，接受者需要将 uvecfd 传递给发送者使用，uvecfd 中包含中断向量值和接收者 UPID 地址（通过 private_data 实现）。
- sys_uintr_register_sender：接受一个 uvecfd，注册成为面向对应接受者、使用相应中断向量的发送者。要成为发送者，如果事先没有初始化 UITT（UITT 每个 task 独有），需要申请内存并初始化 UITT（init_uitt_ctx）。注册时，将一个新的表项写入 UITT，并调用 uintr_set_sender_msrs 更新 MSR 值。
- sys_uintr_unregister_sender：sys_uintr_register_sender 的反向操作，但不会在 UITT 为空时释放内存，因为？？？，内存释放会被推迟到 MM 退出时。
- sys_uintr_register_self：将自身注册为中断向量 vector 的发送者和接受者，不返回文件描述符。
- sys_uintr_register_handler：do_uintr_register_handler 会通过更新 MSR 寄存器的值注册 handler，同时将 UPID 中的通知向量设置为 UINTR_NOTIFICATION_VECTOR，通知目标设置为 smp_processor_id() 的返回值。更新 MSR 寄存器时，handler 地址被写入 MSR_IA32_UINTR_HANDLER，UPID 地址被写入 MSR_IA32_UINTR_PD，OS_ABI_REDZONE 被写入 MSR_IA32_UINTR_STACKADJUST，MSR_IA32_UINTR_MISC 的高 32 与 相或？？？。
- sys_uintr_unregister_handler：sys_uintr_register_handler 的反向操作，清除 MSR 寄存器的值。
- sys_uintr_alt_stack：指定中断处理函数的栈空间，需要传入一个栈指针和栈大小。do_uintr_alt_stack 会将栈地址写入 MSR_IA32_UINTR_STACKADJUST，栈大小忽略？？？，如果传入地址为空，则将 OS_ABI_REDZONE 写入 MSR_IA32_UINTR_STACKADJUST。
- sys_uintr_wait：让当前线程进入可中断的等待状态，直到有中断到达。uintr_receiver_wait 通过 hrtimer 调用实现这一功能（待深入）。