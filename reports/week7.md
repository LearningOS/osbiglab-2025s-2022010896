### 第七周工作总结

左晨阳 2022010896

#### 本周工作

1. 阅读了 uintr-linux-kernel（intel 版）的大部分代码，记录代码阅读笔记（草稿）。

下周展望：

1. 阅读 nimbos 源码，设计 nimbos 中的 uipi 机制。

#### 代码阅读笔记

**arch/x86/include/asm/uintr.h**

定义了用户态中断直接相关的结构：

- uintr_upid：与 intel 手册中 UPID 定义保持一致。UPID 是用户态中断的通信枢纽，由接收者配置，包含中断是否需要通知、通知的 APIC 目标、通知向量以及待处理的中断请求位。
- uintr_upid_ctx：uintr_upid 的上下文信息，包含双向链表指针、接受方 Task 指针、引用计数、活跃标记 receiver_active、进程是否在内核中阻塞导致用户态中断需要等待 waiting、等待的代价由谁承担 waiting_cost。
  - 如果创建时指定 UPID_WAITING_COST_SENDER，则当接收者被交换出 CPU，发送者执行 SENDUIPI 会返回一个错误。
  - 如果创建时指定 UPID_WAITING_COST_RECEIVER，则当接收者被交换出 CPU，发送者执行 SENDUIPI 会导致接受者被唤醒。
- uintr_uitt_entry：与 intel 手册中 UITTE 保持一致，是 UITT 中的表项，描述一个中断发起行为。包含中断向量（64种取值，对应 UPID 中的 64 个请求位）、UPID 地址以及有效位。
- uintr_uitt_ctx：UITT 表结构，需要互斥访问，包含互斥锁、uintr_uitt_entry 指针、引用计数、每一个表项对应的 uintr_upid_ctx 指针以及表项占用位图 uitt_mask。

**arch/x86/kernel/uintr.c**

学习笔记：

1. 每个 task 都可以有各自的 UITT，但是也可以共享（导致内存管理更高效，但也更复杂）。
2. 每个 task 只能注册一个 uintr handler。
3. 系统调用返回文件描述符，可以比较方便地在用户 task 间传递内核私有的数据结构。

内存管理：

alloc_upid 和 free_upid 函数实现了 UPID 的分配和释放，始终将 uintr_upid 包装在 uintr_upid_ctx 中。初始时引用计数为 1，task 指向 current，receiver_active 为 true（将始终为 true，直到 uintr_free 被调用），waiting 为 false。

相应地，alloc_uitt 和 free_uitt 实现了 UITT 的分配和释放，UITT 同样以 uintr_uitt_ctx 为管理单元，其中只需要特殊处理锁的初始化和释放。

实现了 check_upid_ref、put_upid_ref、get_upid_ref、put_uitt_ref、check_uitt_ref、get_uitt_ref 等 UPID 和 UITT 引用计数管理函数。实际上整个文件中诸多代码都是为了正确实现引用计数的管理，下不再赘述。

APIC 相关：

- 这不是实现 uintr 的必要条件。增加这些代码是为了实现让内核可以通过用户态中断的类似机制通知用户程序，例如在 IO_URING 完成时。
- uintr_notify_receiver：接受一个中断向量 uvec 和 uintr_upid_ctx 指针，将 uvec 写入目标 UPID 请求位中，并通过 APIC->send_UINTR 发送中断通知。
- uintr_notify：接受一个 uvecfd file，解析出参数并调用 uintr_notify_receiver。

杂项：

- do_uintr_register_vector：将某中断向量 uvec 加入到 uintr_upid_ctx.uvec_mask 中，表示当前进程准备处理该类型中断。只有已经注册有 handler 的 task 才可以注册中断向量。
- do_uintr_unregister_vector：并不会修改 uintr_upid_ctx.uvec_mask，只会降低 upid_ctx 的引用计数。

- uintr_set_sender_msrs：更新保存的 MSR 寄存器值，加入 UITT 相关配置项。具体而言，UITT 地址被写入 MSR_IA32_UINTR_TT。UITT 表项有效 mask（UINTR_MAX_UITT_NR-1）被写入 MSR_IA32_UINTR_MISC 低 32 为。

- uintr_wait_list：全局链表，等待中的用户态中断 uintr_upid_ctx。由于 read()、sleep() 等系统调用，要接收用户态中断的程序可能正在内核中被阻塞，用户态中断只能在进程被重新调度的时候被处理。为了避免这种情况，发送一个内核中断，内核会唤醒接收者，此过程需要查找 uintr_wait_list，因为此时不再有硬件支持。
  - uintr_switch_to_kernel_interrupt：将通知向量更换为 UINTR_KERNEL_VECTOR，并将 task 的 uintr_upid_ctx 加入 uintr_wait_list，从而从用户态中断变为内核态中断。
  - uintr_remove_task_wait：uintr_switch_to_kernel_interrupt 的逆过程。
  - uintr_wake_up_process：在内核态中断中执行，唤醒用户态中断的目标接受者 task。（待深入）

上下文切换相关：

- switch_uintr_prepare：任务切换时，被切换出的 task 需要先抑制用户态中断的通知，除非启用了 CONFIG_X86_UINTR_BLOCKING，此时调用 uintr_switch_to_kernel_interrupt，将 uintr_upid_ctx 加入 uintr_wait_list。
- switch_uintr_return：在即将返回用户态时调用，当内核通过 XSAVES 保存用户态上下文时，会清空 MSR_IA32_UINTR_MISC 中的 UINV（User Interrupt Notification Vector） 字段（bit 39:32），必须要在这里恢复，因为可能不会自动恢复。此外：
  1. 要更新 UPID ndst 为当前新的 CPU 硬件目标标识。
  2. 要清除 UPID status 中的 UINTR_UPID_STATUS_SN 位，允许中断投递。
  3. 如果进程在抢占期间收到了用户态中断（upid->puir 非零），通过发送自处理器中断（Self-IPI）主动触发硬件检测逻辑。

进程退出时，如下函数被调用进行清理：

- uintr_free：清空 uintr 涉及的 MSR 寄存器。如果当前进程是接受者，先将 UPID 中 SN 位置 1 抑制中断发生，然后移除 uintr_wait_list 中当前 task 的全部 uintr_upid_ctx，将 receiver_active 置为 false。

系统调用：

- sys_uintr_ipi_fd：返回一个文件描述符 uipi_fd，其中包含当前进程的 uitt_ctx，可以在用户进程间传递。该文件描述符绑定到操作 uipifd_fops。uipifd_open 中，如果 mm->context.uitt_ctx 为空，将其更新为 file->private_data（系统调用创建文件描述符时，会读取 mm->context.uitt_ctx 作为 file->private_data）。uipifd_ioctl 支持的操作只有 UIPI_SET_TARGET_TABLE，即依据文件描述符 private_data 设置当前进程的 UITT，并调用 uintr_set_sender_msrs。
- sys_uintr_vector_fd：注册自身想要处理的中断向量并返回文件描述符 uvecfd，接受者需要将 uvecfd 传递给发送者使用，uvecfd 中包含中断向量值和接收者 UPID 地址（通过 private_data 指向 uvecfd_ctx 实现）。
- sys_uintr_register_sender：接受一个 uvecfd，注册成为面向对应接受者、使用相应中断向量的发送者。要成为发送者，如果事先没有初始化 UITT（UITT 每个 task 独有），需要申请内存并初始化 UITT（init_uitt_ctx）。注册时，将一个新的表项写入 UITT，并调用 uintr_set_sender_msrs 更新 MSR 值。该调用返回 UITTE 的下标，可以用于执行 `SENDUIPI <uipi_index>`。
- sys_uintr_unregister_sender：sys_uintr_register_sender 的反向操作，但不会在 UITT 为空时释放内存，因为其他 task 可能共享这一 UITT（但无法验证），此时必须保持共享关系，内存释放会被推迟到 MM 退出时。
- sys_uintr_register_self：将自身注册为中断向量 vector 的发送者和接受者，不返回文件描述符。
- sys_uintr_register_handler：do_uintr_register_handler 会通过更新 MSR 寄存器的值注册 handler，同时将 UPID 中的通知向量设置为 UINTR_NOTIFICATION_VECTOR，通知目标设置为 smp_processor_id() 的返回值。更新 MSR 寄存器时，handler 地址被写入 MSR_IA32_UINTR_HANDLER，UPID 地址被写入 MSR_IA32_UINTR_PD，OS_ABI_REDZONE 被写入 MSR_IA32_UINTR_STACKADJUST，MSR_IA32_UINTR_MISC 的高 32 与 相或？？？。
- sys_uintr_unregister_handler：sys_uintr_register_handler 的反向操作，清除 MSR 寄存器的值。
- sys_uintr_alt_stack：指定中断处理函数的栈空间，需要传入一个栈指针和栈大小。do_uintr_alt_stack 会将栈地址写入 MSR_IA32_UINTR_STACKADJUST，栈大小忽略？？？，如果传入地址为空，则将 OS_ABI_REDZONE 写入 MSR_IA32_UINTR_STACKADJUST。（使用原来的栈可能导致溢出，或者运行时需要其他处理？？？）（应该不是必要的）
- sys_uintr_wait：让当前线程进入可中断的等待状态，直到有中断到达。uintr_receiver_wait 通过 hrtimer 调用实现这一功能（待深入）。

**arch/x86/kernel/irq.c**

uintr_spurious_interrupt：

- 可能的一种情况是，虽然中断发送时接收者正在目标 CPU 上运行，但中断被处理时该 task 已经被交换出去，因此内核是有可能收到 UINTR_NOTIFICATION_VECTOR 的中断的。

- 因此，内核需要处理 UINTR_NOTIFICATION_VECTOR 的中断，此时只需要清空本地 APIC。因为 UPID 会被硬件设置，当接收者 task 被重新调度时，可以接收到中断信息。如果 CONFIG_X86_UINTR_BLOCKING 被设置，还要从 uintr_wait_list 中唤醒 task。

uintr_kernel_notification：

- CONFIG_X86_UINTR_BLOCKING 被设置时，唤醒 uintr_wait_list 中的 task。

**arch/x86/kernel/process_64.c**

__switch_to 中，增加 switch_uintr_prepare（对于换出的 task）和 switch_uintr_finish（对于换进来的 task）调用。

**arch/x86/kernel/signal.c**

arch_do_signal_or_restart：返回用户态前，当进程是 UINTR 接收者，并且有用户态中断打断系统调用时，返回一个 `-EINTR` 告知用户调用被打断，而不试图重启系统调用。

**arch/x86/kernel/trap.c**

senduipi_decode_index: 解码 SENDUIPI 指令的操作数，从指令编码中提取 UITTE 索引值。
- ModR/M 是 x86/x86-64 指令编码中的一个关键字节，用于指定指令的操作数。insn_get_modrm_rm_off 用于解析 ModRM 字节中的 R/M 字段，计算操作数在 pt_regs（寄存器状态结构体）中的偏移量。

fixup_uintr_gp_exception：当用户态执行 SENDUIPI 指令发送中断，但目标 UPID 被标记为阻塞（Blocked）时，硬件会触发 #GP，内核通过此函数模拟中断发送并唤醒接收方任务。该函数不借助硬件而直接操作 UPID，会通过 insn_fetch_from_user 和 senduipi_decode_index 获取并解码 SENDUIPI 指令。

fixup_senduipi_ud_exception：如果 task 已有 UITT（可能是继承来的）但未注册为发送者，导致 SENDUIPI 引发 #UD 异常，该函数调用 uintr_set_sender_msrs 修复异常。该函数在 exc_invalid_op 错误处理中被调用，返回 true 表示修复成功。

**arch/x86/include/asm/mmu_context.h**

destroy_context 和 uitt_dup_context 增加了对 UITT 上下文的处理。uitt_dup_context 中，直接复制 mm->context.uitt_ctx 指针。销毁时，调用 uintr_destroy_uitt_ctx 降低 uitt_ctx 引用计数。

**arch/x86/kernel/process.c**

arch_dup_task_struct 中，thread.upid_ctx 置为 NULL，thread.upid_activated 和 thread.uitt_activated 置为 False。

exit_thread 中，调用 uintr_free 清理 uintr 相关资源。

**arch/x86/kernel/fpu/core.c**

uintr 相关状态不应当被继承，fpu_clone 中需要将 XFEATURE_UINTR 清空。fpu_clone 在 copy_thread 中被调用。

**arch/x86/kernel/cpu/common.c**

setup_uintr 调用 cr4_set_bits 设置 CR4 寄存器的 UINTR 位。这是为了 /proc/cpuinfo 中的 uintr 标志。

