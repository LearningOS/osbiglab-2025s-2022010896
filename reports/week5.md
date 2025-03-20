### 第五周工作总结

左晨阳 2022010896

#### 本周工作

- 阅读 `x86 senduipi` 文档，学习用户态中断的基本概念、设计思想和使用方法；
- 精读 *Obtaining hard real-time performance and rich Linux features in a compounded real-time operating system by a partitioning hypervisor*
- 配置基本开发环境，在qemu上运行 RVM1.5 + nimbos，体验复合实时操作系统的设计思路；
- 阅读 RVM1.5 和 jailhouse 驱动部分源码；

下一周规划：

- 深入阅读 nimbos 源码，理解系统调用是如何被转发给 Linux 处理的；
- 构想系统调用转发的优化方案，减少转发的开销，包括如何利用用户态中断 senduipi，以减少内核态切换的开销；

### 学习笔记

#### x86 senduipi

用户态中断是一种允许用户态程序直接向其他用户态程序发送中断的机制。这种机制避免了传统中断需要通过内核态的中断处理程序，从而减少了上下文切换的开销，提高了中断处理的效率。用户态中断通常用于高性能计算、实时系统等场景，其中低延迟和高吞吐量是关键需求。

SENDUIPI是x86架构中的一条指令，用于发送用户态中断处理器：

- 每个处理器都有一个用户态中断处理队列，用于存储接收到的用户态中断请求。SENDUIPI指令会将中断请求放入目标处理器的这个队列中。
- SENDUIPI指令需要指定一个中断向量，该向量用于标识中断的类型或来源。目标处理器会根据这个向量来调用相应的用户态中断处理程序。
- 由于SENDUIPI指令避免了内核态的中断处理流程，因此可以显著减少中断处理的延迟，提高系统的整体性能。

UPID（User Posted-Interrupt Descriptor） 是 Intel 处理器中用于管理用户中断（User Interrupts）的关键数据结构。它主要由内核维护，也会被硬件自动更新。其中主要字段包括：

- PIF（Posted-Interrupt Requests）：发布中断请求，每个用户中断向量对应一个位，初始化为 0，表示没有待处理的中断请求。
- NV（Notification Vector）：设置为操作系统定义的通知中断向量。
- NDST（Notification Destination）：设置为目标处理器的 APIC ID。
- ON：未完成通知位（Outstanding Notification）。如果该位为1，表示有一个或多个用户中断在 PIR（Posted-Interrupt Requests） 字段中有未完成的通知。
- SN：通知抑制位（Suppress Notification）。如果该位为1，表示代理（包括 SENDUIPI 指令）在发布用户中断时不应发送通知。

SENDUIPI 指令的执行步骤如下：

- 访问 UITT 表：根据指令的寄存器操作数，索引到 UITT 表中的对应 UITTE。
- 检查 UITTE 的有效性：如果 UITTE 的 Bit 0 (V) 为1，表示该表项有效，继续执行；否则，指令无效。
- 获取 UPID 地址：从 UITTE 的 Bits 127:64 (UPIDADDR) 字段中获取 UPID 的线性地址。
- 访问 UPID：根据 UPID 地址，读取 UPID 的内容。
- 发布用户中断：根据 UITTE 中的 Bits 15:8 (UV) 字段（用户中断向量），在 UPID 的 Bits 127:64 (PIF) 字段中设置对应的位，表示该用户中断有请求。
- 发送通知中断：如果 UPID 的 Bit 1 (SN) 为0（表示不抑制通知），并且 Bit 0 (ON) 为1（表示有未完成的通知），则 SENDUIPI 会发送一个普通的 IPI（处理器间中断），使用 UPID 中的 Bits 23:16 (NV) 字段作为通知向量，Bits 63:32 (NDST) 字段作为目标 APIC ID。

https://www.felixcloutier.com/x86/senduipi

#### 论文阅读

该研究提出了一种复合实时操作系统（cRTOS），通过虚拟化程序创建了两个域，分别用于运行普通的 Linux 内核和硬件实时操作系统内核。该设计的优势在于：

- cRTOS 可以同时提供硬实时性能和丰富的 Linux 特性，能够通过X窗口系统执行具有图形用户界面的复杂Linux可执行文件，这使得实时应用程序的开发者可以在属性的开发环境和工具链中进行开发，而直接进行硬实时性能的验证；
- 无需对Linux进行任何修改，不需要重新编译内核；
- 支持完全抢占的硬实时性能，同时达到更良好的时序精度和中断延迟；

**方法**

每个实时进程在 Linux 中有一个对应的影子进程，以便使用 Linux 系统调用。

系统调用处理，将系统调用分为三类处理：

1. 实时系统调用：直接在实时域中处理，例如任务相关的 `clone()` 和 `fork()` 等，时间相关的 `clock_nanosleep()` 和 `gettime()` 等；
2. 远程系统调用：非关键系统调用，例如 IPC 相关和 IO 相关调用，发送到 Linux 域中处理；
3. 双端系统调用：为了保证内存排布的一致性，并确保影子进程和实时进程同时结束，`mmap()`、`munmap()`、`exit()`、`fork()` 需要在两个域中同时执行；

实现中，通过Jailhouse的共享内存和virtio队列来实现远程系统调用

```markdown
  **实时域**：

  - 实时进程的用户线程发起系统调用，切换到其内核线程。
  - 内核线程获取参数，生成请求消息并将其放入virtio队列。
  - 内核线程通过处理器间中断（IPI）通知Linux。
  - 内核线程休眠并让出CPU给下一个可运行线程。

  **普通域**：

  - Linux内核处理IPI，中断处理器获取请求消息并将其放入对应影子进程的内存中，唤醒影子进程中的线程。
  - 影子进程的线程提取请求消息并向Linux发起系统调用。
  - Linux内核执行系统调用。
  - 线程生成包含返回值的回复消息并返回Linux内核。
  - Linux内核将回复消息放入virtio队列，并根据实时领域线程的优先级决定是否通过IPI通知Nuttx内核。

  **实时域**：
  - Nuttx内核从队列中获取回复消息，并唤醒发起远程系统调用的内核线程。
  - 内核线程从回复消息中获取返回值并返回给用户线程。
```

启动时，首先正常启动 Linux，然后启动Jailhouse虚拟化，将Linux迁移到正常域运行。最后启动Nuttx，将其移动到实时域。

#### RVM1.5 + nimbos 运行

- 问题：编译nimbos时遇到 x86_64-linux-musl-gcc 编译器未安装，apt 找不到该包；
  - 解决：手动下载预编译的 musl 工具链 https://musl.cc/x86_64-linux-musl-cross.tgz ，解压并添加到环境变量。
- 问题：配置开发环境，安装 `cargo-binutils --vers =0.3.3` 遇到错误：
  ```
  [ERROR nimbos::lang_items] Panicked at src/task/structs.rs:131 new_user: no such app 
  ```
  - 解决：暂时移除 rust-toolchain 文件对 nightly-2022-02-22 的强制要求，安装完开发工具后，再恢复版本覆盖编译 nimbos。
- 问题：qemu 内 Ubuntu 初次启动发生 `Failed to start OpenBSD Secure Shell server`，且无法通过ssh连接；
  - 解决：根据 https://askubuntu.com/questions/1223825/failed-to-start-openbsd-secure-shell-server 重装 `ssh server/client` 解决。
- 问题：nimbos-driver 加载后，rtos不可以正常启动，会在运行中发生
  ```
  panicked at 'Unhandled exception #0xd', src/arch/x86_64/exception.rs:66:13
  ```
  - 解决：咨询助教后，添加 RVM=on nimbos 编译选项。
- 问题：nimbos 运行时，发生
  ```
  [ERROR nimbos::lang_items] Panicked at src/task/structs.rs:131 new_user: no such app
  ```
  - 解决：编译nimbos时，先执行 make user，编译用户态程序，再执行 make 编译内核。

#### RVM1.5 和 jailhouse 驱动部分源码

RVM1.5 是一种 Type 1.5 型虚拟机，它在 Type 1 型虚拟机的基础上增加了对硬件的直接访问，以提高性能。

- RVM 1.5 启动时，所有普通域 cpu 上都启动一个进程进入 main 函数。而一个实时域的核心会被 jailhouse 暂时停止，等待 rtos 启动。
- 从 main 进入后，每个 CPU 核心都会携带自己的 PerCpu 数据结构和 Linux 栈指针（linux_sp）作为参数。主核心会进行初始化，这包括初始化日志系统、初始化内存管理系统（堆、页表等）和初始化虚拟机的基本结构 cell，期间其他核心会等待。
- 随后，每个核心（包括主核心和其他核心）都会初始化自己的 PerCpu 数据结构，并调用 `cpu_data.init(linux_sp, cell::root_cell())` 来完成核心特定的初始化工作。这包括重新加载 Linux 的上下文，以及将构建的页表在 cpu 上激活。
- 所有核心在完成初始化后，会调用 `cpu_data.activate_vmm()` 来激活虚拟机监控模式（VMM），进入虚拟机的运行状态，此时用户将看到 Linux 恢复执行，只不过此时 Linux 运行在虚拟机中，可以调用虚拟机的 hypercall，并且此时 Linux 只使用比原本少一个 cpu 的核心数。

在启动 rtos 时，需要由 nimbos 的驱动程序调用 RVM1.5 的系统调用接口。hypercall 函数会根据调用码，进入 start_rtos 函数。

- 函数首先通过 ap_start 和 ap_end 获取 AP 启动代码的地址范围，并将这段代码复制到预先分配的启动页面（START_PAGE_PADDR）中。启动页面的最后一个位置存储了入口地址（entry_paddr），用于指定 AP 启动后跳转的目标地址。
- 函数遍历所有可能的 APIC ID，检查哪些 CPU 核心尚未启动（apic_to_cpu_id(apic_id) == u32::MAX）。对于未启动的核心，函数设置其栈顶地址并调用 apic::start_ap 启动该核心。启动后，函数通过检查 PerCpu::entered_cpus() 是否增加，等待新核心完成初始化。
- 核心启动完成后，函数恢复启动页面的原始数据并返回成功状态，此时 rtos 已经在原本暂停的核心上运行。