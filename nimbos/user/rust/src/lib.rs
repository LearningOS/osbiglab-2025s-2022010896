#![no_std]
#![feature(linkage)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(panic_info_message)]

#[macro_use]
pub mod console;

mod arch;
mod lang_items;
mod syscall;
use core::arch::asm;

#[repr(C)]
pub struct TimeSpec {
    /// seconds
    pub sec: usize,
    /// nano seconds
    pub nsec: usize,
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    exit(main());
}

#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

use syscall::*;

pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

pub fn sched_yield() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn exec(path: &str) -> isize {
    sys_exec(path)
}

pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => {
                sched_yield();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => {
                sched_yield();
            }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn sleep(period_ms: usize) {
    sys_nanosleep(&TimeSpec {
        sec: period_ms / 1000,
        nsec: (period_ms % 1000) * 1_000_000,
    });
}

pub fn thread_spawn(entry: fn(usize) -> i32, arg: usize) -> usize {
    use core::sync::atomic::{AtomicUsize, Ordering};
    const MAX_THREADS: usize = 16;
    const THREAD_STACK_SIZE: usize = 4096 * 4; // 16K
    static mut THREAD_STACKS: [[u8; THREAD_STACK_SIZE]; MAX_THREADS] =
        [[0; THREAD_STACK_SIZE]; MAX_THREADS];
    static THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);

    let thread_id = THREAD_COUNT.fetch_add(1, Ordering::AcqRel);
    let newsp = unsafe { THREAD_STACKS[thread_id].as_ptr_range().end as usize };
    sys_clone(entry, arg, newsp)
}

/// 开启中断 UIF
#[inline(always)]
pub fn stui() {
    unsafe {asm!("stui", options(nostack))};
}

/// 关闭中断 UIF
#[inline(always)]
pub fn clui() {
    unsafe {asm!("clui", options(nostack))};
}

pub fn uintr_register_sender(upid_addr: usize, uvec: u8) -> isize {
    sys_uintr_register_sender(upid_addr, uvec)
}

pub fn uintr_register_handler(handler: usize) -> usize {
    sys_uintr_register_handler(handler)
}

// 发送用户中断
#[inline(always)]
pub unsafe fn senduipi(upid_addr: u64) {
    asm!(
        "senduipi rax",
        in("rax") upid_addr,
        options(nostack),
    );
}

#[macro_export]
macro_rules! make_uintr_entry {
    ($name:ident, $handler:ident) => {
        #[naked]
        pub unsafe extern "C" fn $name() {
            asm!(
                // CFI 指令用于调试和栈展开
                ".cfi_startproc",
                ".cfi_signal_frame",
                ".cfi_def_cfa rsp, 32",
                ".cfi_offset rsp, -8",
                ".cfi_offset rip, -24",
        
                // 跳过填充 (8字节)
                "sub rsp, 8",
                ".cfi_adjust_cfa_offset 8",
        
                // 设置 xsave_area 为 NULL
                "push 0",
                ".cfi_adjust_cfa_offset 8",
        
                // 保存所有通用寄存器到栈上 (构建中断帧)
                "push rax",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rax, 0",
        
                "push r15",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r15, 0",
        
                "push r14",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r14, 0",
        
                "push r13",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r13, 0",
        
                "push r12",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r12, 0",
        
                "push rbp",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rbp, 0",
        
                "push rbx",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rbx, 0",
        
                "push r11",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r11, 0",
        
                "push r10",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r10, 0",
        
                "push r9",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r9, 0",
        
                "push r8",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset r8, 0",
        
                "push rcx",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rcx, 0",
        
                "push rdx",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rdx, 0",
        
                "push rsi",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rsi, 0",
        
                "push rdi",
                ".cfi_adjust_cfa_offset 8",
                ".cfi_rel_offset rdi, 0",
        
                // 设置第一个参数为中断帧指针
                "mov rdi, rsp",
        
                // 对齐栈指针 (16字节对齐)
                "sub rsp, 8",
                ".cfi_adjust_cfa_offset 8",
        
                // 调用 Rust 中断处理函数
                concat!("call ", stringify!($handler)),
        
                // 恢复栈指针
                "add rsp, 8",
                ".cfi_adjust_cfa_offset -8",
                
                // 恢复所有通用寄存器
                "pop rdi",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rsi",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rdx",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rcx",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r8",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r9",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r10",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r11",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rbx",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rbp",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r12",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r13",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r14",
                ".cfi_adjust_cfa_offset -8",
                
                "pop r15",
                ".cfi_adjust_cfa_offset -8",
                
                "pop rax",
                ".cfi_adjust_cfa_offset -8",
        
                // 移除 pad, uirrv 和 xsave_area (共24字节)
                "add rsp, 24",
                ".cfi_adjust_cfa_offset -24",
        
                // 用户中断返回指令
                "uiret",
                "nop",
        
                ".cfi_endproc",
                
                options(noreturn)
            )
        }
    };
}