use core::arch::asm;

use x86_64::registers::rflags::RFlags;

use super::gdt::{UCODE64_SELECTOR, UDATA_SELECTOR};
use crate::arch::instructions;
use crate::mm::{PhysAddr, VirtAddr};
use crate::percpu::PerCpu;
use crate::syscall::uintr::{UintrUittCtx, UintrUpidCtx};
use alloc::sync::Arc;
use crate::sync::Mutex;
use alloc::boxed::Box;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TrapFrame {
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Pushed by 'vector.S'
    pub vector: u64,
    pub error_code: u64,

    // Pushed by CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,

    // Pushed by CPU when trap from ring-3
    pub user_rsp: u64,
    pub user_ss: u64,
}

impl TrapFrame {
    pub fn new_user(entry: VirtAddr, ustack_top: VirtAddr, arg0: usize) -> Self {
        Self {
            rdi: arg0 as _,
            rip: entry.as_usize() as _,
            cs: UCODE64_SELECTOR.0 as _,
            rflags: RFlags::INTERRUPT_FLAG.bits(), // IOPL = 0, IF = 1
            user_rsp: ustack_top.as_usize() as _,
            user_ss: UDATA_SELECTOR.0 as _,
            ..Default::default()
        }
    }

    pub const fn new_clone(&self, ustack_top: VirtAddr) -> Self {
        let mut tf = *self;
        // cs, user_ss are not pushed into TrapFrame in syscall_entry
        tf.cs = UCODE64_SELECTOR.0 as _;
        tf.user_ss = UDATA_SELECTOR.0 as _;
        tf.user_rsp = ustack_top.as_usize() as _;
        tf.rax = 0; // for child thread, clone returns 0
        tf
    }

    pub const fn new_fork(&self) -> Self {
        let mut tf = *self;
        // cs, user_ss are not pushed into TrapFrame in syscall_entry
        tf.cs = UCODE64_SELECTOR.0 as _;
        tf.user_ss = UDATA_SELECTOR.0 as _;
        tf.rax = 0; // for child process, fork returns 0
        tf
    }

    pub fn is_user(&self) -> bool {
        self.cs & 0b11 == 3
    }

    pub unsafe fn exec(&self, kstack_top: VirtAddr) -> ! {
        info!(
            "user task start: entry={:#x}, ustack={:#x}, kstack={:#x}",
            self.rip,
            self.user_rsp,
            kstack_top.as_usize(),
        );
        instructions::disable_irqs();
        assert_eq!(
            PerCpu::current().arch_data().as_ref().kernel_stack_top(),
            kstack_top
        );
        asm!("
            mov     rsp, {tf}
            pop     rax
            pop     rcx
            pop     rdx
            pop     rbx
            pop     rbp
            pop     rsi
            pop     rdi
            pop     r8
            pop     r9
            pop     r10
            pop     r11
            pop     r12
            pop     r13
            pop     r14
            pop     r15
            add     rsp, 16     // pop vector, error_code
            swapgs
            iretq",
            tf = in(reg) self,
            options(noreturn),
        )
    }
}

#[repr(C)]
#[derive(Debug, Default)]
struct ContextSwitchFrame {
    msr98a: u64,
    msr989: u64,
    msr988: u64,
    msr987: u64,
    msr986: u64,
    uif: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rip: u64,
}

#[derive(Debug)]
pub struct TaskContext {
    pub kstack_top: VirtAddr,
    pub rsp: u64,
    pub fs_base: u64,
    pub cr3: u64,
    pub uitt: Option<Arc<Mutex<UintrUittCtx>>>,
    pub uitt_activated: bool,
    pub upid_activated: bool,
    pub uintr_upid_ctx: Option<Box<UintrUpidCtx>>,
}

impl TaskContext {
    pub const fn default() -> Self {
        // unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
        Self {
            kstack_top: VirtAddr::new(0),
            rsp: 0,
            fs_base: 0,
            cr3: 0,
            uitt: None,  // 关键修复：确保 Option<Arc> 是 None 而不是非法值
            uitt_activated: false,
            upid_activated: false,
            uintr_upid_ctx: None,
        }
    }

    pub fn init(
        &mut self,
        entry: usize,
        kstack_top: VirtAddr,
        page_table_root: PhysAddr,
        _is_kernel: bool,
    ) {
        unsafe {
            let frame_ptr = (kstack_top.as_mut_ptr() as *mut ContextSwitchFrame).sub(1);
            core::ptr::write(
                frame_ptr,
                ContextSwitchFrame {
                    rip: entry as _,
                    ..Default::default()
                },
            );
            self.rsp = frame_ptr as u64;
        }
        self.kstack_top = kstack_top;
        self.cr3 = page_table_root.as_usize() as u64;
        self.uitt_activated = false;
        self.upid_activated = false;
        self.uintr_upid_ctx = None;
    }

    pub fn switch_to(&mut self, next_ctx: &Self) {
        unsafe {
            PerCpu::current()
                .arch_data()
                .as_mut()
                .set_kernel_stack_top(next_ctx.kstack_top);
            instructions::set_user_page_table_root(next_ctx.cr3 as usize);
            // TODO: swtich fs_base
            context_switch(&mut self.rsp, &next_ctx.rsp)
        }
    }
}

#[naked]
unsafe extern "C" fn context_switch(_current_stack: &mut u64, _next_stack: &u64) {
    asm!(
        "
        push    rbp
        push    rbx
        push    r12
        push    r13
        push    r14
        push    r15
        
        // Save UIF (TESTUI -> CF, then store CF in AL and push)
        testui
        setc    al
        push    rax         // Push 8 bytes (but only AL is used)

        // Save MSRs (5 x 64-bit = 40 bytes total)
        sub     rsp, 40     // Allocate space for MSRs (5x8 bytes)
        mov     ecx, 0x00000986
        rdmsr
        mov     [rsp + 32], eax    // Store EAX (low 32 bits)
        mov     [rsp + 36], edx    // Store EDX (high 32 bits)
        
        mov     ecx, 0x00000987
        rdmsr
        mov     [rsp + 24], eax
        mov     [rsp + 28], edx
        
        mov     ecx, 0x00000988
        rdmsr
        mov     [rsp + 16], eax
        mov     [rsp + 20], edx
        
        mov     ecx, 0x00000989
        rdmsr
        mov     [rsp + 8], eax
        mov     [rsp + 12], edx
        
        mov     ecx, 0x0000098a
        rdmsr
        mov     [rsp], eax
        mov     [rsp + 4], edx

        // Switch stacks
        mov     [rdi], rsp

        mov     rsp, [rsi]

        // Restore MSRs (reverse order)
        mov     ecx, 0x0000098a
        mov     eax, [rsp]
        mov     edx, [rsp + 4]
        wrmsr
        
        mov     ecx, 0x00000989
        mov     eax, [rsp + 8]
        mov     edx, [rsp + 12]
        wrmsr
        
        mov     ecx, 0x00000988
        mov     eax, [rsp + 16]
        mov     edx, [rsp + 20]
        wrmsr
        
        mov     ecx, 0x00000987
        mov     eax, [rsp + 24]
        mov     edx, [rsp + 28]
        wrmsr
        
        mov     ecx, 0x00000986
        mov     eax, [rsp + 32]
        mov     edx, [rsp + 36]
        wrmsr

        add     rsp, 40     // Free MSR storage

        // Restore UIF (pop saved CF, then set UIF)
        pop     rax         // AL = saved UIF (CF)
        test    al, al
        jz      1f
        stui
        jmp     2f
    1:
        clui
    2:

        pop     r15
        pop     r14
        pop     r13
        pop     r12
        pop     rbx
        pop     rbp
        ret",
        options(noreturn),
    )
}
