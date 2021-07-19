use array_macro::array;

use alloc::boxed::Box;
use core::{fmt::Display, mem};

use crate::consts::{MAXPATH, MAXARG, MAXARGLEN};
use super::{Proc, elf};

pub type SysResult = Result<usize, ()>;

pub trait Syscall {
    fn sys_exec(&mut self) -> SysResult;
}

impl Syscall for Proc {
    fn sys_exec(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        if let Err(s) = self.arg_str(0, &mut path) {
            syscall_warning(s);
            return Err(())
        }

        let mut result: SysResult = Err(());
        let mut error = "too many arguments";
        let mut uarg: usize;
        let uargv = self.arg_addr(1);
        let mut argv: [Option<Box<[u8; MAXARGLEN]>>; MAXARG] = array![_ => None; MAXARG];
        for i in 0..MAXARG {
            // fetch ith arg's address into uarg
            match self.fetch_addr(uargv+i*mem::size_of::<usize>()) {
                Ok(addr) => uarg = addr,
                Err(s) => {
                    error = s;
                    break
                },
            }
            if uarg == 0 {
                match elf::load(self, &path, &argv[..i]) {
                    Ok(ret) => result = Ok(ret),
                    Err(s) => error = s,
                }
                break       
            }

            // allocate kernel space to copy in user arg
            match Box::try_new_zeroed() {
                Ok(b) => unsafe { argv[i] = Some(b.assume_init()) },
                Err(_) => {
                    error = "not enough kernel memory";
                    break
                },
            }

            // copy user arg into kernel space
            if let Err(s) = self.fetch_str(uarg, argv[i].as_deref_mut().unwrap()) {
                error = s;
                break
            }
        }

        if result.is_err() {
            syscall_warning(error);
        }
        result
    }
}

// LTODO - switch to macro that can include line numbers
#[inline]
fn syscall_warning<T: Display>(s: T) {
    #[cfg(feature = "kernel_warning")]
    println!("syscall waring: {}", s);
}
