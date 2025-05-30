const SYSCALL_READ: usize = 0;
const SYSCALL_WRITE: usize = 1;
const SYSCALL_YIELD: usize = 24;
const SYSCALL_NANOSLEEP: usize = 35;
const SYSCALL_GETPID: usize = 39;
const SYSCALL_CLONE: usize = 56;
const SYSCALL_FORK: usize = 57;
const SYSCALL_EXEC: usize = 59;
const SYSCALL_EXIT: usize = 60;
const SYSCALL_WAITPID: usize = 61;
const SYSCALL_GET_TIME_MS: usize = 96;
const SYSCALL_CLOCK_GETTIME: usize = 228;
const SYSCALL_UINTR_REGISTER_SENDER: usize = 333;
const SYSCALL_UINTR_REGISTER_HANDLER: usize = 334;

mod fs;
mod task;
mod time;
pub mod uintr;

#[cfg(feature = "rvm")]
use crate::scf::syscall::*;

#[cfg(not(feature = "rvm"))]
use self::fs::*;

use self::task::*;
use self::time::*;
use self::uintr::*;
use crate::arch::{instructions, TrapFrame};

pub fn syscall(
    tf: &mut TrapFrame,
    syscall_id: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
) -> isize {
    instructions::enable_irqs();
    debug!(
        "syscall {} enter <= ({:#x}, {:#x}, {:#x})",
        syscall_id, arg0, arg1, arg2
    );
    let ret = match syscall_id {
        SYSCALL_READ => sys_read(arg0, arg1.into(), arg2),
        SYSCALL_WRITE => sys_write(arg0, arg1.into(), arg2),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_NANOSLEEP => sys_nanosleep(arg0.into()),
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_CLONE => sys_clone(arg0, tf),
        SYSCALL_FORK => sys_fork(tf),
        SYSCALL_EXEC => sys_exec(arg0.into(), tf),
        SYSCALL_EXIT => sys_exit(arg0 as i32),
        SYSCALL_WAITPID => sys_waitpid(arg0 as isize, arg1.into()),
        SYSCALL_GET_TIME_MS => sys_get_time_ms(),
        SYSCALL_CLOCK_GETTIME => sys_clock_gettime(arg0, arg1.into()),
        SYSCALL_UINTR_REGISTER_SENDER => sys_uintr_register_sender(arg0 as _, arg1 as _),
        SYSCALL_UINTR_REGISTER_HANDLER => sys_uintr_register_handler(arg0 as _) as _,
        _ => {
            println!("Unsupported syscall_id: {}", syscall_id);
            crate::task::CurrentTask::get().exit(-1);
        }
    };
    debug!("syscall {} ret => {:#x}", syscall_id, ret);
    instructions::disable_irqs();
    ret
}
