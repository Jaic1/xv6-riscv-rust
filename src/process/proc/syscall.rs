use array_macro::array;

use alloc::string::String;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::{fmt::Display, mem};

use crate::consts::{MAXPATH, MAXARG, MAXARGLEN};
use crate::fs::File;
use crate::mm::Address;

use super::{Proc, elf};

pub type SysResult = Result<usize, ()>;

pub trait Syscall {
    fn sys_exec(&mut self) -> SysResult;
    fn sys_dup(&mut self) -> SysResult;
    fn sys_open(&mut self) -> SysResult;
    fn sys_write(&mut self) -> SysResult;
}

impl Syscall for Proc {
    /// Load an elf binary and execuate it the currrent process context.
    fn sys_exec(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;

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

        #[cfg(feature = "trace_syscall")]
        println!("exec({}, {:#x}) = {:?}", String::from_utf8_lossy(&path), uargv, result);

        if result.is_err() {
            syscall_warning(error);
        }
        result
    }

    /// Duplicate a file descriptor.
    fn sys_dup(&mut self) -> SysResult {
        let old_fd = self.arg_fd(0)?;
        let pd = self.data.get_mut();
        let new_fd = pd.alloc_fd().ok_or(())?;
        
        let old_file = pd.open_files[old_fd].as_ref().unwrap();
        let new_file = Arc::clone(old_file);
        let none_file = pd.open_files[new_fd].replace(new_file);
        debug_assert!(none_file.is_none());

        #[cfg(feature = "trace_syscall")]
        println!("dup({}) = {}(fd)", old_fd, new_fd);

        Ok(new_fd)
    }

    /// Open and optionally create a file.
    /// Note1: It can only possibly create a regular file,
    ///     use [`sys_mknod`] to creata special file instead.
    /// Note2: File permission and modes are not supported yet.
    fn sys_open(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;
        let flags = self.arg_i32(1);
        if flags < 0 {
            return Err(())
        }

        let fd = self.data.get_mut().alloc_fd().ok_or(())?;
        let file = File::open(&path, flags).ok_or(())?;
        let none_file = self.data.get_mut().open_files[fd].replace(file);
        debug_assert!(none_file.is_none());

        #[cfg(feature = "trace_syscall")]
        println!("open({}, {:#x}) = {}(fd)", String::from_utf8_lossy(&path), flags, fd);

        Ok(fd)
    }

    /// Write user content to file descriptor.
    /// Return the conut of bytes written.
    fn sys_write(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let addr = self.arg_addr(1);
        let count = self.arg_raw(2);
        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = file.fwrite(Address::Virtual(addr), count);

        #[cfg(feature = "trace_syscall")]
        println!("write({}, {:#x}, {}) = {:?}", fd, addr, count, ret);

        ret
    }
}

// LTODO - switch to macro that can include line numbers
#[inline]
fn syscall_warning<T: Display>(s: T) {
    #[cfg(feature = "kernel_warning")]
    println!("syscall waring: {}", s);
}
