use libvmm::msr::Msr;

use super::cpuid::CpuId;

pub fn frequency() -> u16 {
    static CPU_FREQUENCY: spin::Once<u16> = spin::Once::new();
    *CPU_FREQUENCY.call_once(|| {
        const DEFAULT: u16 = 4000;
        CpuId::new()
            .get_processor_frequency_info()
            .map(|info| info.processor_base_frequency())
            .unwrap_or(DEFAULT)
            .max(DEFAULT)
    })
}

/// 安全、可移植的 `__rdtscp` 手动实现
#[inline(always)]
pub unsafe fn rdtscp_manual(aux: &mut u32) -> u64 {
    let high: u32;
    let low: u32;
    
    // 使用 `asm!` 宏直接生成 `rdtscp` 指令
    // 完全匹配 `core::arch::x86_64::__rdtscp` 的行为：
    // 1. 执行 `rdtscp` 指令
    // 2. 将 TSC 值返回到 RAX (低32位) 和 RDX (高32位)
    // 3. 将 `aux` 值写入 ECX 并存储到传入的内存地址
    println!("manual start");
    core::arch::asm!(
        "rdtscp",
        // 输出操作数：
        out("eax") low,      // TSC 低32位 → low
        out("edx") high,     // TSC 高32位 → high
        out("ecx") *aux,     // aux 值 → 内存地址
        // 选项：
        options(nostack, nomem, preserves_flags)
    );
    
    println!("manual end");
    
    // 组合高低32位为 u64
    ((high as u64) << 32) | (low as u64)
}

pub fn current_cycle() -> u64 {
    let mut aux = 0;
    // println!("current_cycle");
    // let res = unsafe { core::arch::x86_64::__rdtscp(&mut aux) };
    // let res = unsafe {rdtscp_manual(&mut aux)};
    // println!("current_cycle end");
    // res
    0
}

pub fn current_time_nanos() -> u64 {
    current_cycle() * 1000 / frequency() as u64
    // 0
}

pub fn thread_pointer() -> usize {
    let ret;
    unsafe { core::arch::asm!("mov {0}, gs:0", out(reg) ret, options(nostack)) }; // PerCpu::self_vaddr
    ret
}

pub fn set_thread_pointer(tp: usize) {
    unsafe { Msr::IA32_GS_BASE.write(tp as u64) };
}
