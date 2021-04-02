use crate::process;
use crate::trap::get_time_ms;

pub fn sys_exit(exit_code: i32) -> isize {
    process::exit(exit_code);
    0
}

pub fn sys_yield() -> isize {
    process::suspend();
    0
}

pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

pub fn sys_getpid() -> isize {
    process::getpid() as isize
}

pub fn sys_fork() -> isize {
    process::fork() as isize
}
