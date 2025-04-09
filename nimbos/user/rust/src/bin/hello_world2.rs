#![no_std]
#![no_main]
#![feature(naked_functions)]

#[macro_use]
extern crate user_lib;

use user_lib::{uintr_register_sender, uintr_register_handler, stui, senduipi, fork, exit, waitpid};
use core::sync::atomic::{AtomicBool, Ordering};

static INTERRUPT_RECEIVED: AtomicBool = AtomicBool::new(false);

/// 中断处理函数
#[no_mangle]
pub extern "C" fn naked_uintr_handler() {
    INTERRUPT_RECEIVED.store(true, Ordering::SeqCst);
    println!("[Parent] Received UIPI interrupt in user mode");
    stui();
}

make_uintr_entry!(uintr_handler, naked_uintr_handler);

#[no_mangle]
pub fn main() -> i32 {
    println!("[Parent] Starting parent process");

    // 1. 父进程注册中断处理函数
    let handler_address = uintr_handler as usize;
    let upid_addr = uintr_register_handler(handler_address);
    println!("[Parent] Registered handler, UPID address: {:x}", upid_addr);

    // 2. 创建子进程
    println!("[Parent] Forking child process...");
    let pid = fork();
    if pid == 0 {
        // 子进程代码
        println!("[Child] I am the child process");

        // 3. 子进程注册发送者
        println!("[Child] Registering as sender...");
        let entry = uintr_register_sender(upid_addr, 1);
        if entry < 0 {
            println!("[Child] Sender register failed: {}", entry);
            exit(-1);
        }
        println!("[Child] Registered as sender, entry: {}", entry);

        // 4. 子进程发送中断
        println!("[Child] Sending UIPI to parent...");
        unsafe { senduipi(entry.try_into().unwrap()) };
        println!("[Child] UIPI sent successfully");

        exit(0)
    } else {
        // 父进程代码
        println!("[Parent] Child process created with PID: {}", pid);

        // 启用中断接收
        stui();
        println!("[Parent] Enabled interrupt reception (STUI executed)");

        // 5. 父进程等待中断
        println!("[Parent] Waiting for interrupt...");
        loop {
            if INTERRUPT_RECEIVED.load(Ordering::SeqCst) {
                println!("[Parent] Interrupt flag set, processing...");
                break;
            }
        }

        // 6. 等待子进程退出
        let mut exit_code = 0;
        let wait_pid = waitpid(pid as usize, &mut exit_code);
        println!("[Parent] Child process {} exited with code {}", wait_pid, exit_code);
    }

    println!("[Parent] Program completed successfully");
    0
}