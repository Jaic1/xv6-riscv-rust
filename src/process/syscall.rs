use core::{mem, ptr, str};

use crate::consts::{MAXPATH, MAXARG};

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
                if let Ok(s) = str::from_utf8(&path) {
                    println!("sys_exec: {}", s);
                } else {
                    panic!("sys_exec: unkown path");
                }
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

        // TODO
        
        panic!("sys_exec: end");
    }
}

impl Proc {
    fn arg_raw(&self, n: usize) -> usize {
        let tf = unsafe {&*self.tf};
        match n {
            0 => {tf.get_a0()}
            1 => {tf.get_a1()}
            2 => {tf.get_a2()}
            3 => {tf.get_a3()}
            4 => {tf.get_a4()}
            5 => {tf.get_a5()}
            _ => {
                panic!("argraw: n is larger than 5");
            }
        }
    }

    fn arg_addr(&self, n: usize) -> *const usize {
        self.arg_raw(n) as *const usize
    }

    fn arg_str(&self, n: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let addr: usize = self.arg_raw(n);
        self.pagetable.as_ref().unwrap().copy_in_str(addr, buf)?;
        Ok(())
    }
}
