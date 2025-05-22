### 第十四周工作总结

左晨阳 2022010896

#### 本周工作总结

修复所有已知错误，包括：

- qemu-uitr 中 xsave 未保存和恢复 UIF；
- qemu-uintr 未面向 uintr-linux-kernel 支持 xsave；
- shadow 程序 signal handler 中对 uintr 相关的 MSR 修改不会被 xsave 保留；

目前，能够正常使用 uintr 进行内核态 Syscall Forwarding。

#### 下周工作计划

- 使用 uintr 实现从 Linux 用户程序到 Nimbos 内核的反向通知；
- 优化当前对 SCF buffer 和 irqnum 资源的使用；
- 完善实现文档，准备报告 slide；

#### 问题与解决

**XSAVE：MSR 保存及恢复**

上一周工作中，总结发现存在 MSR 被误清零导致的 uintr 收发异常。本周进一步分析 qemu 日志，发现当进程被调度到其他处理器时，MSR 和 UIF 状态错误，而当进程重新被调度至原 CPU 后，MSR 和 UIF 状态恢复正常。在 qemu 中增加日志输出，发现 xsave 未被调用。在 Linux 中进一步检查日志，发现 xsave 未被启用，并使用 fxsave 作为替代。由于只有 xsave 支持 uintr 相关状态的保存和恢复，必须启用 xsave。

- Linux 默认支持 uintr 的处理器同样支持相关 xsave，而 qemu 未正确处理相关配置，导致错误较为隐蔽，难以发现；

qemu 中，在 `target/i386/cpu.c` 内置了多个型号的处理器配置。切换不同型号的处理器，观察 xsave 的启用情况，发现关键配置为 `features[FEAT_1_ECX]` 中的 `CPUID_EXT_XSAVE`。在支持 uintr 的处理器中增加该配置，成功在 Linux 中启用 xsave。

然而，目前 qemu 仍然不完全支持 uintr 相关的 xsave。具体而言，Linux 将 uintr_state 归类为 supervisor 状态，要求其状态的保存采用 Compact 模式，而 qemu 中的实现为标准的 Uncompact 模式。直接运行会导致以下错误：

```bash
[    0.000000] x86/fpu: xstate features 0x4003
[    0.000000] x86/fpu: xstate features 0x4003
[    0.000000] ------------[ cut here ]------------
[    0.000000] No fixed offset for xstate 14
[    0.000000] WARNING: CPU: 0 PID: 0 at arch/x86/kernel/fpu/xstate.c:445 fpu__init_system_xstate+0x8f2/0x9a1
[    0.000000] Modules linked in:
[    0.000000] CPU: 0 PID: 0 Comm: swapper Not tainted 5.15.0-rc1+ #180
[    0.000000] RIP: 0010:fpu__init_system_xstate+0x8f2/0x9a1
[    0.000000] Code: a3 e0 73 2d 83 cd ff 80 3d ee 5c b1 ff 00 0f 85 ee f9 ff ff 44 89 e6 48 c7 c7 98 14 b4 9
[    0.000000] RSP: 0000:ffffffff94e03e58 EFLAGS: 00000082 ORIG_RAX: 0000000000000000
[    0.000000] RAX: 0000000000000000 RBX: 0000000000000000 RCX: c0000000ffffdfff
[    0.000000] RDX: ffffffff94e03c90 RSI: 00000000ffffdfff RDI: 0000000000000000
[    0.000000] RBP: 00000000ffffffff R08: 0000000000000000 R09: ffffffff94e03c88
[    0.000000] R10: 0000000000000001 R11: 0000000000000001 R12: 000000000000000e
[    0.000000] R13: 0000000000000030 R14: 0000000000004003 R15: 0000000000000240
[    0.000000] FS:  0000000000000000(0000) GS:ffffffff9550f000(0000) knlGS:0000000000000000
[    0.000000] CS:  0010 DS: 0000 ES: 0000 CR0: 0000000080050033
[    0.000000] CR2: ffff88800008b000 CR3: 00000003c6988000 CR4: 00000000000406a0
[    0.000000] Call Trace:
[    0.000000]  ? fpu__init_system+0x111/0x14e
[    0.000000]  ? early_cpu_init+0x393/0x3bd
[    0.000000]  ? setup_arch+0x53/0xb71
[    0.000000]  ? start_kernel+0x64/0x69d
[    0.000000]  ? copy_bootdata+0xf/0x44
[    0.000000]  ? secondary_startup_64_no_verify+0xc2/0xcb
[    0.000000] random: get_random_bytes called from print_oops_end_marker+0x21/0x40 with crng_init=0
[    0.000000] ---[ end trace 0000000000000000 ]---
[    0.000000] ------------[ cut here ]------------
[    0.000000] XSAVE consistency problem, dumping leaves
```

此外，Linux 不会为用户程序保存 supervisor 状态。因此，目前对 Linux kernel 进行修改，将 uintr_state 归类为 user 状态。在 `arch/x86/kernel/fpu/xstate.c` 中，将 `XFEATURE_MASK_UINTR` 从 `XFEATURE_MASK_SUPERVISOR_SUPPORTED` 移动到 `XFEATURE_MASK_USER_SUPPORTED`，成功修复了 Linux 对 uintr_state 的保存和恢复问题。

**UIF 的保存和恢复**

目前，qemu 不会在 xsave 时保存 UIF，但根据 Linux 的实现，UIF 应当占用一个 UINTR_MISC 的保留位，在 xsave 时保存。因此，在 qemu 中进行修改：

```c
// target/i386/tcg/fpu_helper.c
static void do_xsave_uintr(CPUX86State *env, target_ulong ptr, uintptr_t ra) {
    // ...
    cpu_stq_data_ra(env, ptr + offsetof(XSaveUINTR, misc_uif), 
        env->uintr_misc + (env->uintr_uif << 63), ra);
    // ...
}

static void do_xrstor_uintr(CPUX86State *env, target_ulong ptr, uintptr_t ra) {
    // ...
    uint64_t temp = cpu_ldq_data_ra(env, ptr + offsetof(XSaveUINTR, misc_uif), ra);
    env->uintr_uif = temp >> 63;
    env->uintr_misc = (temp << 1) >> 1;
    // ...
}
```

这样，成功修复了 Linux 下运行 uipi_sample 的所有错误。

**Syscall Forwarding**

启用了 xsave 后，发现原系统调用转发完全无法使用。进一步检查发现，在 shadow 程序中，会在 signal handler 中进行 uintr 相关系统调用从而注册 uintr handler。查阅资料知，signal handler 可能不会保留其中修改的 MSR 状态，也就是不会在进入和退出 signal handler 时执行 xrstor 和 xsave。因此，在 shadow 程序中，uintr 相关注册的第一次执行移至 signal handler 之外，成功修复了该问题。由于此后的 syscall 处理会在 uintr handler 中运行，不会再需要特殊处理。

```c
int nimbos_setup_syscall_buffers(int nimbos_fd, int slot_num, int *uintr_fd, uint64_t *upid_addr)
{
    _stui();
    int err = uintr_register_handler(uintr_handler, 0);
    if (err) {
        fprintf(stderr, "Interrupt handler register error\n");
        return err;
    }

    *uintr_fd = uintr_create_fd(0, 0);
    if (*uintr_fd < 0) {
        fprintf(stderr, "Interrupt vector allocation error\n");
        return *uintr_fd;
    }
    err = ioctl(*uintr_fd, UINTR_GET_UPID_PHYS_ADDR, upid_addr);
    if (err < 0) {
        fprintf(stderr, "ioctl failed\n");
        close(*uintr_fd);
        return err;
    }
    // 原始逻辑
}
```
