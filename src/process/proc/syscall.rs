use array_macro::array;

use alloc::string::String;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::{fmt::Display, mem};

use crate::consts::{MAXPATH, MAXARG, MAXARGLEN};
use crate::process::PROC_MANAGER;
use crate::fs::{self, File};
use crate::mm::Address;

use super::{Proc, elf};

pub type SysResult = Result<usize, ()>;

pub trait Syscall {
    fn sys_fork(&mut self) -> SysResult;
    fn sys_exit(&mut self) -> SysResult;
    fn sys_wait(&mut self) -> SysResult;
    fn sys_read(&mut self) -> SysResult;
    fn sys_exec(&mut self) -> SysResult;
    fn sys_fstat(&mut self) -> SysResult;
    fn sys_dup(&mut self) -> SysResult;
    fn sys_sbrk(&mut self) -> SysResult;
    fn sys_open(&mut self) -> SysResult;
    fn sys_write(&mut self) -> SysResult;
    fn sys_close(&mut self) -> SysResult;
}

impl Syscall for Proc {
    /// Redirect to [`fork`].
    ///
    /// [`fork`]: Proc::fork
    fn sys_fork(&mut self) -> SysResult {
        let ret = self.fork();

        #[cfg(feature = "trace_syscall")]
        println!("[{}].fork() = {:?}(pid)", self.excl.lock().pid, ret);

        ret
    }

    /// Exit the current process normally.
    /// Note: This function call will not return.
    fn sys_exit(&mut self) -> SysResult {
        let exit_status = self.arg_i32(0);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].exit(status={})", self.excl.lock().pid, exit_status);

        unsafe { PROC_MANAGER.exiting(self.index, exit_status); }
        unreachable!("process exit");
    }

    /// Wait for any child(if any) process to exit.
    /// Recycle the chile process and return its pid.
    fn sys_wait(&mut self) -> SysResult {
        let addr = self.arg_addr(0);
        let ret =  unsafe { PROC_MANAGER.waiting(self.index, addr) };

        #[cfg(feature = "trace_syscall")]
        println!("[pid={}].wait(addr={:#x}) = {:?}(pid)", self.excl.lock().pid, addr, ret);

        ret
    }

    /// Read form file descriptor.
    fn sys_read(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let addr = self.arg_addr(1);
        let count = self.arg_i32(2);
        if count <= 0 {
            return Err(())
        }
        
        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = file.fread(Address::Virtual(addr), count as usize);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].read(fd={}, addr={:#x}, count={}) = {:?}", self.excl.lock().pid, fd, addr, count, ret);

        ret
    }
    
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

    /// Given a file descriptor, return the file status.
    fn sys_fstat(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let addr = self.arg_addr(1);
        let mut stat = fs::FileStat::uninit();
        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = if file.fstat(&mut stat).is_err() {
            Err(())
        } else {
            let pgt = self.data.get_mut().pagetable.as_mut().unwrap();
            if pgt.copy_out(&stat as *const fs::FileStat as *const u8, addr, mem::size_of::<fs::FileStat>()).is_err() {
                Err(())
            } else {
                Ok(0)
            }
        };

        #[cfg(feature = "trace_syscall")]
        println!("[{}].fstat(fd={}, addr={:#x}) = {:?}", self.excl.lock().pid, fd, addr, stat);

        ret
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
        println!("[{}].dup({}) = {}(fd)", self.excl.lock().pid, old_fd, new_fd);

        Ok(new_fd)
    }

    /// Redirect to [`sbrk`].
    ///
    /// [`sbrk`]: ProcData::sbrk
    fn sys_sbrk(&mut self) -> SysResult {
        let increment = self.arg_i32(0);
        let ret = self.data.get_mut().sbrk(increment);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].sbrk({}) = {:?}", self.excl.lock().pid, increment, ret);

        ret
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
        println!("[{}].open({}, {:#x}) = {}(fd)", self.excl.lock().pid, String::from_utf8_lossy(&path), flags, fd);

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
        println!("[{}].write({}, {:#x}, {}) = {:?}", self.excl.lock().pid, fd, addr, count, ret);

        ret
    }

    /// Given a file descriptor, close the opened file.
    fn sys_close(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let file = self.data.get_mut().open_files[fd].take();

        #[cfg(feature = "trace_syscall")]
        println!("[{}].close(fd={}), file={:?}", self.excl.lock().pid, fd, file);

        drop(file);
        Ok(0)
    }
}

// LTODO - switch to macro that can include line numbers
#[inline]
fn syscall_warning<T: Display>(s: T) {
    #[cfg(feature = "kernel_warning")]
    println!("syscall waring: {}", s);
}
