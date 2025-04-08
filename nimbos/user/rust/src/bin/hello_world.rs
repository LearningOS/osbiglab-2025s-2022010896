#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

#[macro_use]
extern crate user_lib;

use user_lib::{uintr_register_sender, uintr_register_handler};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

static INTERRUPT_RECEIVED: AtomicBool = AtomicBool::new(false);

// 中断处理函数
#[no_mangle]
pub extern "C" fn uintr_handler() {
    loop {

    }
    // INTERRUPT_RECEIVED.store(true, Ordering::SeqCst);
    // println!("Received interrupt in user mode");
}

// 发送用户中断
pub unsafe fn senduipi(upid_addr: u64) {
    asm!(
        "mov rax, {upid_addr}",
        ".byte 0xf3",
        ".byte 0x0f",
        ".byte 0xc7",
        ".byte 0xf0",
        upid_addr = in(reg) upid_addr,
        options(nostack),
    );
}

// 用户中断返回
pub unsafe fn uiret() {
    asm!(
        ".byte 0xf3",
        ".byte 0x0f",
        ".byte 0x01",
        ".byte 0xec",
        options(nostack),
    );
}

// 开启中断
pub unsafe fn stui() {
    asm!(
        ".byte 0xf3",
        ".byte 0x0f",
        ".byte 0x01",
        ".byte 0xef",
        options(nostack),
    );
}


#[no_mangle]
pub fn main() -> i32 {
    println!("senduipi: {:x}", senduipi as u64);
    println!("Hello world from user mode program!");

    // 1. 注册中断处理函数
    let handler_address = uintr_handler as usize;
    // let handler_address = 0 as usize;
    let upid_addr = uintr_register_handler(handler_address);
    println!("upid_addr: {:x}", upid_addr);

    unsafe {stui();}

    // 3. 发送中断
    let entry = uintr_register_sender(upid_addr);
    if entry < 0 {
        println!("Sender register failed: {}", entry);
        return -1;
    }
    println!("Sender register success, entry: {}", entry);

    unsafe {senduipi(entry.try_into().unwrap())};

    // 循环等待中断发生
    let mut i: u64 = 0;
    loop {
        // 检查全局变量，如果收到中断，则跳出循环
        if INTERRUPT_RECEIVED.load(Ordering::SeqCst) {
            break;
        }
        if i % 100000000 == 0 {
            println!("Waiting for interrupt... {}", i);
        }
        i += 1;
    }

    // 4. 打印完成信息
    println!("Done!");
    0
}
