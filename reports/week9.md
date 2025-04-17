### 第九周工作总结

左晨阳 2022010896

#### 完善 nimbos uintr 支持

**UIF 的保存和恢复**

在上下文切换时，保存和恢复 UIF 的状态：

```rust
    // Save UIF (TESTUI -> CF, then store CF in AL and push)
    testui
    setc    al
    push    rax         // Push 8 bytes (but only AL is used)
```

```rust
    // Restore UIF (pop saved CF, then set UIF)
    pop     rax         // AL = saved UIF (CF)
    test    al, al
    jz      1f
    stui
    jmp     2f
1:
    clui
2:
```

发现 testui 不能正确读取 UIF 的值，检查 uintr-qemu 发现模拟器未实现该指令。尝试在 QEMU 中实现该指令，按照 Intel 手册规范，进行如下 EFLAGS 赋值操作：

```c
CF := UIF;
ZF := AF := OF := PF := SF := 0;
```

具体代码中，修改指令翻译：

```c
case 0xed:
    if (prefixes & PREFIX_REPZ){ /* TESTUI */
        gen_helper_testui(cpu_env);
        set_cc_op(s, CC_OP_EFLAGS);
    }
    break;
```

具体执行如下：

```c
void helper_testui(CPUX86State *env){
    if(uif_enable(env)){
        cpu_load_eflags(env, CC_C, CC_O | CC_S | CC_Z | CC_A | CC_P | CC_C);
    }else{
        cpu_load_eflags(env, 0, CC_O | CC_S | CC_Z | CC_A | CC_P | CC_C);
    }
}
```

测试发现，现在可以正确读取 UIF 的值。

**用户程序读取中断向量值**

根据中断帧的排布，定义 `TrapFrame` 结构体，用户中断处理函数以一个 `TrapFrame` 的引用作为参数，从而可以读取被硬件压入的中断向量值：

```rust
...
[  2.693659 WARN  nimbos::syscall::uintr][0:5] sys_uintr_register_sender called
[  2.693937 WARN  nimbos::syscall::uintr][0:5] Registering sender: uvec=0x8, upid=0xffffff800069ce40
[  2.694411 WARN  nimbos::syscall::uintr][0:5] UITT entry 8 registered
Sender register success, entry: 8
uitte index:8
uitt addr: 0xffffff800069d221  upid addr: 0xffffff800069ce40
senduipi core: 0 uitte index:8  dist core: 0 ifsend: 1, nv: 236
receive, cur core:0
Received interrupt in user mode, uvec: 8
XXXuiret 
Interrupt received 9 times
...
```

#### qemu-uintr 运行 cRTOS

目前 nimbos 使用 APIC 物理目标模式，而 Linux 会使用逻辑目标模式。查阅资料未发现从 qemu 或者 Linux 中强制使用物理目标模式的方法，因此决定在 nimbos 中实现逻辑目标模式。

具体来说，Linux 使用的是 Flat model，此时 LDR (logical destination register) 中 24-31 位是目标处理器 Bitmap，而低 24 位为保留位，因此在 `apic.rs` 中添加：
    
```rust
const APIC_LDR_OFFSET: u32 = 24;

pub fn get_apic_id() -> u32 {
    unsafe { LOCAL_APIC.as_ref().id() >> APIC_LDR_OFFSET }
}

pub fn get_logical_dest() -> u32 {
    let apic_id = get_apic_id();
    let logical_dest = 1 << (apic_id + APIC_LDR_OFFSET);
    logical_dest
}
```

初始化中，需要设置当前处理器的逻辑目标值：

```rust
unsafe {
    LOCAL_APIC.as_mut().set_logical_id(get_logical_dest());
}
```

后续发送中断时也要使用逻辑目标值，但设置 UPID 时，仍然使用 APIC ID。

#### 跨域 uintr

