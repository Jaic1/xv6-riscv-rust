use core::mem;

use crate::consts::{MAXPATH};
use super::proc::Proc;

pub trait Syscall {
    fn sys_exec(&mut self) -> usize;
}

impl Syscall for Proc {
    fn sys_exec(&mut self) -> usize {
        let mut path: [u8; MAXPATH] = unsafe {
            mem::MaybeUninit::uninit().assume_init()
        };
        match self.arg_str(0, &mut path) {
            Ok(_) => {
                // debug
                print!("sys_exec: ");
                for c in path.iter() {
                    if *c == 0 {
                        break
                    }
                    print!("{}", *c as char);
                }
                println!();
            }
            Err(str) => {
                println!("sys_exec1: {}", str);
                return usize::MAX;
            }
        }

        // tmp ignore argv, i.e., only have path as first argument
        // also not copy any content from user to kernel
        // let mut argv: [*mut u8; MAXARG] = [ptr::null_mut(); MAXARG];
        // let argv_addr: *const usize = self.arg_addr(1);
        // argv[0] = unsafe {*argv_addr} as *mut u8;
        // argv[1] = unsafe {*argv_addr.offset(1)} as *mut u8;
        // if argv[1] as usize != 0 {
        //     panic!("argv[1] is not 0, is {}...", unsafe {*argv[1]});
        // }
        // argv[1] = ptr::null_mut();

        // TODO - ELF load
        panic!("sys_exec: end");
    }
}
