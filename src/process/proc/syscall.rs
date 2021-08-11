use array_macro::array;

use alloc::string::String;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::convert::TryInto;
use core::fmt::Display;
use core::mem;

use crate::consts::{MAXPATH, MAXARG, MAXARGLEN, fs::MAX_DIR_SIZE};
use crate::process::PROC_MANAGER;
use crate::fs::{ICACHE, Inode, InodeType, LOG, File, Pipe, FileStat};
use crate::trap;

use super::{Proc, elf};

pub type SysResult = Result<usize, ()>;

pub trait Syscall {
    fn sys_fork(&mut self) -> SysResult;
    fn sys_exit(&mut self) -> SysResult;
    fn sys_wait(&mut self) -> SysResult;
    fn sys_pipe(&mut self) -> SysResult;
    fn sys_read(&mut self) -> SysResult;
    fn sys_kill(&mut self) -> SysResult;
    fn sys_exec(&mut self) -> SysResult;
    fn sys_fstat(&mut self) -> SysResult;
    fn sys_chdir(&mut self) -> SysResult;
    fn sys_dup(&mut self) -> SysResult;
    fn sys_getpid(&mut self) -> SysResult;
    fn sys_sbrk(&mut self) -> SysResult;
    fn sys_sleep(&mut self) -> SysResult;
    fn sys_uptime(&mut self) -> SysResult;
    fn sys_open(&mut self) -> SysResult;
    fn sys_write(&mut self) -> SysResult;
    fn sys_mknod(&mut self) -> SysResult;
    fn sys_unlink(&mut self) -> SysResult;
    fn sys_link(&mut self) -> SysResult;
    fn sys_mkdir(&mut self) -> SysResult;
    fn sys_close(&mut self) -> SysResult;
}

impl Syscall for Proc {
    /// Redirect to [`Proc::fork`].
    ///
    /// [`Proc::fork`]: Proc::fork
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
        println!("[{}].wait(addr={:#x}) = {:?}(pid)", self.excl.lock().pid, addr, ret);

        ret
    }

    /// Create pipe for user.
    fn sys_pipe(&mut self) -> SysResult {
        let pipefds_addr = self.arg_addr(0);
        let addr_fdread = pipefds_addr;
        let addr_fdwrite = pipefds_addr+mem::size_of::<u32>();

        // alloc fd
        let pdata = self.data.get_mut();
        let (fd_read, fd_write) = pdata.alloc_fd2().ok_or(())?;

        // alloc pipe
        let (file_read, file_write) = Pipe::create().ok_or(())?;

        // transfer fd to user
        let fd_read_u32: u32 = fd_read.try_into().unwrap();
        let fd_write_u32: u32 = fd_write.try_into().unwrap();
        pdata.copy_out(&fd_read_u32 as *const u32 as *const u8, addr_fdread, mem::size_of::<u32>())?;
        pdata.copy_out(&fd_write_u32 as *const u32 as *const u8, addr_fdwrite, mem::size_of::<u32>())?;

        // assign the file to process
        pdata.open_files[fd_read].replace(file_read);
        pdata.open_files[fd_write].replace(file_write);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].pipe(addr={:#x}) = ok, fd=[{},{}]", self.excl.lock().pid, pipefds_addr, fd_read, fd_write);

        Ok(0)
    }

    /// Read form file descriptor.
    fn sys_read(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let user_addr = self.arg_addr(1);
        let count = self.arg_i32(2);
        if count <= 0 || self.data.get_mut().check_user_addr(user_addr).is_err() {
            return Err(())
        }
        let count = count as u32;
        
        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = file.fread(user_addr, count);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].read(fd={}, addr={:#x}, count={}) = {:?}", self.excl.lock().pid, fd, user_addr, count, ret);

        ret.map(|count| count as usize)
    }

    /// Kill a process.
    /// Note: Other signals are not supported yet.
    fn sys_kill(&mut self) -> SysResult {
        let pid = self.arg_i32(0);
        if pid < 0 {
            return Err(())
        }
        let pid = pid as usize;
        let ret = unsafe { PROC_MANAGER.kill(pid) };

        #[cfg(feature = "trace_syscall")]
        println!("[{}].kill(pid={}) = {:?}", self.excl.lock().pid, pid, ret);

        ret.map(|()| 0)
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
        println!("[{}].exec({}, {:#x}) = {:?}", self.excl.lock().pid, String::from_utf8_lossy(&path), uargv, result);

        if result.is_err() {
            syscall_warning(error);
        }
        result
    }

    /// Given a file descriptor, return the file status.
    fn sys_fstat(&mut self) -> SysResult {
        let fd = self.arg_fd(0)?;
        let addr = self.arg_addr(1);
        let mut stat = FileStat::uninit();
        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = if file.fstat(&mut stat).is_err() {
            Err(())
        } else {
            let pgt = self.data.get_mut().pagetable.as_mut().unwrap();
            if pgt.copy_out(&stat as *const FileStat as *const u8, addr, mem::size_of::<FileStat>()).is_err() {
                Err(())
            } else {
                Ok(0)
            }
        };

        #[cfg(feature = "trace_syscall")]
        println!("[{}].fstat(fd={}, addr={:#x}) = {:?}", self.excl.lock().pid, fd, addr, stat);

        ret
    }

    /// Change the current process's working directory,
    fn sys_chdir(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;

        LOG.begin_op();
        let inode: Inode;
        if let Some(i) = ICACHE.namei(&path) {
            inode = i;
        } else {
            LOG.end_op();
            return Err(())
        }
        let idata = inode.lock();
        if idata.get_itype() != InodeType::Directory {
            drop(idata); drop(inode); LOG.end_op();
            return Err(())
        }
        drop(idata);
        let old_cwd = self.data.get_mut().cwd.replace(inode);
        debug_assert!(old_cwd.is_some());
        drop(old_cwd);
        LOG.end_op();
        Ok(0)
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

    /// Get the process's pid.
    fn sys_getpid(&mut self) -> SysResult {
        let pid = self.excl.lock().pid;

        #[cfg(feature = "trace_syscall")]
        println!("[{}].getpid() = {}", pid, pid);

        Ok(pid)
    }

    /// Redirect to [`ProcData::sbrk`].
    ///
    /// [`ProcData::sbrk`]: ProcData::sbrk
    fn sys_sbrk(&mut self) -> SysResult {
        let increment = self.arg_i32(0);
        let ret = self.data.get_mut().sbrk(increment);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].sbrk({}) = {:?}", self.excl.lock().pid, increment, ret);

        ret
    }

    /// Put the current process into sleep.
    fn sys_sleep(&mut self) -> SysResult {
        let count = self.arg_i32(0);
        if count < 0 {
            return Err(())
        }
        let count = count as usize;
        let ret = trap::clock_sleep(self, count);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].sleep({}) = {:?}", self.excl.lock().pid, count, ret);

        ret.map(|()| 0)
    }

    /// Not much like the linux/unix's uptime.
    /// Just return the ticks in current implementation.
    fn sys_uptime(&mut self) -> SysResult {
        let ret = trap::clock_read();

        #[cfg(feature = "trace_syscall")]
        println!("[{}].uptime() = {}", self.excl.lock().pid, ret);

        Ok(ret)
    }

    /// Open and optionally create a file.
    /// Note1: It can only possibly create a regular file,
    ///     use [`Syscall::sys_mknod`] to creata special file instead.
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
        let user_addr = self.arg_addr(1);
        let count = self.arg_i32(2);
        if count <= 0 || self.data.get_mut().check_user_addr(user_addr).is_err() {
            return Err(())
        }
        let count = count as u32;

        let file = self.data.get_mut().open_files[fd].as_ref().unwrap();
        let ret = file.fwrite(user_addr, count);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].write({}, {:#x}, {}) = {:?}", self.excl.lock().pid, fd, user_addr, count, ret);

        ret.map(|count| count as usize)
    }

    /// Create a device file.
    fn sys_mknod(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;
        let major = self.arg_i32(1);
        let minor = self.arg_i32(2);
        if major < 0 || minor < 0 {
            return Err(())
        }

        let major: u16 = major.try_into().map_err(|_| ())?;
        let minor: u16 = minor.try_into().map_err(|_| ())?;
        LOG.begin_op();
        let ret = ICACHE.create(&path, InodeType::Device, major, minor, true).ok_or(());

        #[cfg(feature = "trace_syscall")]
        println!("[{}].mknod(path={}, major={}, minor={}) = {:?}",
            self.excl.lock().pid, String::from_utf8_lossy(&path), major, minor, ret);

        let ret = ret.map(|inode| {drop(inode);0});
        LOG.end_op();
        ret
    }

    /// Delete a pathname and possibly delete the refered inode in the fs.
    /// In essence, [`Syscall::sys_unlink`] will decrement the link count of the inode.
    fn sys_unlink(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;

        LOG.begin_op();
        let mut name: [u8; MAX_DIR_SIZE] = [0; MAX_DIR_SIZE];
        let dir_inode: Inode;
        if let Some(inode) = ICACHE.namei_parent(&path, &mut name) {
            dir_inode = inode;
        } else {
            LOG.end_op();
            return Err(())
        }

        let mut dir_idata = dir_inode.lock();
        let ret = dir_idata.dir_unlink(&name);
        drop(dir_idata);
        drop(dir_inode);
        LOG.end_op();

        #[cfg(feature = "trace_syscall")]
        println!("[{}].unlink(path={}) = {:?}", self.excl.lock().pid, String::from_utf8_lossy(&path), ret);

        ret.map(|()| 0)
    }

    /// Create a new hard link.
    fn sys_link(&mut self) -> SysResult {
        let mut old_path: [u8; MAXPATH] = [0; MAXPATH];
        let mut new_path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut old_path).map_err(syscall_warning)?;
        self.arg_str(1, &mut new_path).map_err(syscall_warning)?;

        LOG.begin_op();

        // find old path
        let old_inode = ICACHE.namei(&old_path).ok_or_else(|| {LOG.end_op(); ()})?;
        let mut old_idata = old_inode.lock();
        let (old_dev, old_inum) = old_idata.get_dev_inum();
        if old_idata.get_itype() == InodeType::Directory {
            syscall_warning("trying to create new link to a directory");
            LOG.end_op();
            return Err(())
        }
        old_idata.link();
        old_idata.update();
        drop(old_idata);

        // if we cannot create a new path
        let revert_link = move |inode: Inode| {
            let mut idata = inode.lock();
            idata.unlink();
            idata.update();
            drop(idata);
            drop(inode);
            LOG.end_op();
        };

        // create new path
        let mut name: [u8; MAX_DIR_SIZE] = [0; MAX_DIR_SIZE];
        let new_inode: Inode;
        match ICACHE.namei_parent(&new_path, &mut name) {
            Some(inode) => new_inode = inode,
            None => {
                revert_link(old_inode);
                return Err(())
            }
        }
        let mut new_idata = new_inode.lock();
        if new_idata.get_dev_inum().0 != old_dev || new_idata.dir_link(&name, old_inum).is_err() {
            revert_link(old_inode);
            return Err(())
        }
        drop(new_idata);
        drop(new_inode);
        drop(old_inode);

        LOG.end_op();

        #[cfg(feature = "trace_syscall")]
        println!("[{}].link(old_path={}, new_path={})", self.excl.lock().pid,
            String::from_utf8_lossy(&old_path), String::from_utf8_lossy(&new_path));
        
        Ok(0)
    }

    /// Create a directory.
    /// Note: Mode is not supported yet.
    fn sys_mkdir(&mut self) -> SysResult {
        let mut path: [u8; MAXPATH] = [0; MAXPATH];
        self.arg_str(0, &mut path).map_err(syscall_warning)?;

        LOG.begin_op();
        let ret = ICACHE.create(&path, InodeType::Directory, 0, 0, false);

        #[cfg(feature = "trace_syscall")]
        println!("[{}].mkdir(path={}) = {:?}", self.excl.lock().pid, String::from_utf8_lossy(&path), ret);

        let ret = match ret {
            Some(inode) => {
                drop(inode);
                Ok(0)
            },
            None => Err(()),
        };
        LOG.end_op();
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
