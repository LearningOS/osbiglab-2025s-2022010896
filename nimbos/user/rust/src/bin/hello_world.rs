#![no_std]
#![no_main]
#![feature(naked_functions)]

#[macro_use]
extern crate user_lib;

use user_lib::{uintr_register_sender, uintr_register_handler, stui, senduipi};
use core::sync::atomic::{AtomicBool, Ordering};

static INTERRUPT_RECEIVED: AtomicBool = AtomicBool::new(false);

/// 中断处理函数
#[no_mangle]
pub extern "C" fn naked_uintr_handler() {
    INTERRUPT_RECEIVED.store(true, Ordering::SeqCst);
    println!("Received interrupt in user mode");
    stui();
}

make_uintr_entry!(uintr_handler, naked_uintr_handler);

#[no_mangle]
pub fn main() -> i32 {
    println!("Hello world from user mode program!");

    // 1. 注册中断处理函数
    let handler_address = uintr_handler as usize;
    let upid_addr = uintr_register_handler(handler_address);
    println!("upid_addr: {:x}", upid_addr);

    // 2. 发送中断
    let entry = uintr_register_sender(upid_addr, 1);
    if entry < 0 {
        println!("Sender register failed: {}", entry);
        return -1;
    }
    println!("Sender register success, entry: {}", entry);

    stui();
    unsafe {senduipi(entry.try_into().unwrap())};

    let mut i = 0;
    // 循环等待中断发生
    while i < 10 {
        // 检查全局变量，如果收到中断，则跳出循环
        if INTERRUPT_RECEIVED.load(Ordering::SeqCst) {
            INTERRUPT_RECEIVED.store(false, Ordering::SeqCst);
            i = i + 1;
            println!("Interrupt received {} times", i);
            unsafe {senduipi(entry.try_into().unwrap())};
        }
    }

    // 4. 打印完成信息
    println!("Done!");
    0
}
