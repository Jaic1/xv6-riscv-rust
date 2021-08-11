use array_macro::array;

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::mem;
use core::sync::atomic::{AtomicBool, Ordering};
use core::option::Option;
use core::ptr;
use core::cell::UnsafeCell;

use crate::consts::{PGSIZE, fs::{NFILE, ROOTIPATH}};
use crate::mm::{PageTable, RawPage, RawSinglePage};
use crate::register::{satp, sepc, sstatus};
use crate::spinlock::{SpinLock, SpinLockGuard};
use crate::trap::user_trap;
use crate::fs::{Inode, ICACHE, LOG, File};

use super::CpuManager;
use super::PROC_MANAGER;
use super::cpu::CPU_MANAGER;
use super::{fork_ret, Context, TrapFrame};

use self::syscall::Syscall;

mod syscall;
mod elf;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum ProcState {
    UNUSED,
    SLEEPING,
    RUNNABLE,
    RUNNING,
    ALLOCATED,
    ZOMBIE,
}

/// Exclusive to the process
pub struct ProcExcl {
    pub state: ProcState,
    pub exit_status: i32,
    pub channel: usize,
    pub pid: usize,
}

impl ProcExcl {
    const fn new() -> Self {
        Self {
            state: ProcState::UNUSED,
            exit_status: 0,
            channel: 0,
            pid: 0,
        }
    }

    /// Clean up the content in [`ProcExcl`],
    pub fn cleanup(&mut self) {
        self.pid = 0;
        self.channel = 0;
        self.exit_status = 0;
        self.state = ProcState::UNUSED;
    }
}

/// Data private to the process
/// Only accessed by the current process when it is running,
/// or initialed by other process(e.g. fork) with ProcExcl lock held
pub struct ProcData {
    kstack: usize,
    sz: usize,
    context: Context,
    name: [u8; 16],
    open_files: [Option<Arc<File>>; NFILE],
    /// trapframe to hold temp user register value, etc
    pub tf: *mut TrapFrame,
    /// user pagetable
    pub pagetable: Option<Box<PageTable>>,
    /// current working directory
    pub cwd: Option<Inode>,
}

impl ProcData {
    const fn new() -> Self {
        Self {
            kstack: 0,
            sz: 0,
            context: Context::new(),
            name: [0; 16],
            open_files: array![_ => None; NFILE],
            tf: ptr::null_mut(),
            pagetable: None,
            cwd: None,
        }
    }

    /// Set kstack
    pub fn set_kstack(&mut self, kstack: usize) {
        self.kstack = kstack;
    }

    /// Init the context of the process after it is created
    /// Set its return address to fork_ret,
    /// which start to return to user space.
    pub fn init_context(&mut self) {
        self.context.clear();
        self.context.set_ra(fork_ret as *const () as usize);
        self.context.set_sp(self.kstack + PGSIZE*4);
    }

    /// Return the process's mutable reference of context
    pub fn get_context(&mut self) -> *mut Context {
        &mut self.context as *mut _
    }

    /// Prepare for the user trap return
    /// Return current proc's satp for assembly code to switch page table
    pub fn user_ret_prepare(&mut self) -> usize {
        let tf: &mut TrapFrame = unsafe { self.tf.as_mut().unwrap() };
        tf.kernel_satp = satp::read();
        // current kernel stack's content is cleaned
        // after returning to the kernel space
        tf.kernel_sp = self.kstack + PGSIZE*4;
        tf.kernel_trap = user_trap as usize;
        tf.kernel_hartid = unsafe { CpuManager::cpu_id() };

        // restore the user pc previously stored in sepc
        sepc::write(tf.epc);

        self.pagetable.as_ref().unwrap().as_satp()
    }

    /// Simply check if the user passed-in virtual address is in range.
    fn check_user_addr(&self, user_addr: usize) -> Result<(), ()> {
        if user_addr > self.sz {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Copy content from src to the user's dst virtual address.
    /// Copy `count` bytes in total.
    /// It will redirect the call to pagetable.
    #[inline]
    pub fn copy_out(&mut self, src: *const u8, dst: usize, count: usize) -> Result<(), ()> {
        self.pagetable.as_mut().unwrap().copy_out(src, dst, count)
    }

    /// Copy content from the user's src virtual address to dst.
    /// Copy `count` bytes in total.
    /// It will redirect the call to pagetable.
    #[inline]
    pub fn copy_in(&self, src: usize, dst: *mut u8, count: usize) -> Result<(), ()> {
        self.pagetable.as_ref().unwrap().copy_in(src, dst, count)
    }

    /// Allocate a new file descriptor.
    /// The returned fd could be used directly to index, because it is private to the process.
    fn alloc_fd(&mut self) -> Option<usize> {
        self.open_files.iter()
            .enumerate()
            .find(|(_, f)| f.is_none())
            .map(|(i, _)| i)
    }

    /// Allocate a pair of file descriptors.
    /// Typically used for pipe creation.
    fn alloc_fd2(&mut self) -> Option<(usize, usize)> {
        let mut iter = self.open_files.iter()
            .enumerate()
            .filter(|(_, f)| f.is_none())
            .take(2)
            .map(|(i, _)| i);
        let fd1 = iter.next()?;
        let fd2 = iter.next()?;
        Some((fd1, fd2))
    }

    /// Clean up the content in [`ProcData`],
    /// except kernel stack, context, opened files and cwd.
    /// LTODO - should excl must be held by caller during this cleanup?
    pub fn cleanup(&mut self) {
        self.name[0] = 0;
        let tf = self.tf;
        self.tf = ptr::null_mut();
        if !tf.is_null() {
            unsafe { RawSinglePage::from_raw_and_drop(tf as *mut u8); }
        }
        let pgt = self.pagetable.take();
        if let Some(mut pgt) = pgt {
            pgt.dealloc_proc_pagetable(self.sz);
        }
        self.sz = 0;
    }

    /// Close any opened files and cwd,
    /// except kernel stack and context.
    /// Should only be called when the process exits.
    pub fn close_files(&mut self) {
        for f in self.open_files.iter_mut() {
            drop(f.take())
        }
        LOG.begin_op();
        debug_assert!(self.cwd.is_some());
        drop(self.cwd.take());
        LOG.end_op();
    }

    /// Increase/Decrease the user program break for the process.
    /// Return the previous program break if succeed.
    fn sbrk(&mut self, increment: i32) -> Result<usize, ()> {
        let old_size = self.sz;
        if increment > 0 {
            let new_size = old_size + (increment as usize);
            self.pagetable.as_mut().unwrap().uvm_alloc(old_size, new_size)?;
            self.sz = new_size;
        } else if increment < 0 {
            let new_size = old_size - ((-increment) as usize);
            self.pagetable.as_mut().unwrap().uvm_dealloc(old_size, new_size);
            self.sz = new_size;
        }
        Ok(old_size)
    }
}

/// Process Struct
/// 
/// LTODO - ProcData could be protected by RefCell,
/// but in case when the process is mutating the ProcData,
/// but then if it is interrupted and get killed, so it need to
/// clean its ProcData, so UnsafeCell is better.
pub struct Proc {
    /// index into the process table
    index: usize,
    pub excl: SpinLock<ProcExcl>,
    pub data: UnsafeCell<ProcData>,
    pub killed: AtomicBool,
}

impl Proc {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            excl: SpinLock::new(ProcExcl::new(), "ProcExcl"),
            data: UnsafeCell::new(ProcData::new()),
            killed: AtomicBool::new(false),
        }
    }

    /// Called by ProcManager's user_init,
    /// Only be called once for the first user process
    pub fn user_init(&mut self) {
        let pd = self.data.get_mut();

        // map initcode in user pagetable
        pd.pagetable.as_mut().unwrap().uvm_init(&INITCODE);
        pd.sz = PGSIZE;

        // prepare return pc and stack pointer
        let tf = unsafe { pd.tf.as_mut().unwrap() };
        tf.epc = 0;
        tf.sp = PGSIZE;

        let init_name = b"initcode\0";
        unsafe {
            ptr::copy_nonoverlapping(
                init_name.as_ptr(), 
                pd.name.as_mut_ptr(),
                init_name.len()
            );
        }

        debug_assert!(pd.cwd.is_none());
        pd.cwd = Some(ICACHE.namei(&ROOTIPATH).expect("cannot find root inode by b'/'"));
    }

    /// Abondon current process if
    /// the killed flag is true
    pub fn check_abondon(&mut self, exit_status: i32) {
        if self.killed.load(Ordering::Relaxed) {
            unsafe { PROC_MANAGER.exiting(self.index, exit_status); }
        }
    }

    /// Abondon current process by:
    /// 1. setting its killed flag to true
    /// 2. and then exit
    pub fn abondon(&mut self, exit_status: i32) {
        self.killed.store(true, Ordering::Relaxed);
        unsafe { PROC_MANAGER.exiting(self.index, exit_status); }
    }

    /// Handle system call
    /// It may be interrrupted in the procedure of syscall
    pub fn syscall(&mut self) {
        sstatus::intr_on();

        let tf = unsafe { self.data.get_mut().tf.as_mut().unwrap() };
        let a7 = tf.a7;
        tf.admit_ecall();
        let sys_result = match a7 {
            1 => self.sys_fork(),
            2 => self.sys_exit(),
            3 => self.sys_wait(),
            4 => self.sys_pipe(),
            5 => self.sys_read(),
            6 => self.sys_kill(),
            7 => self.sys_exec(),
            8 => self.sys_fstat(),
            9 => self.sys_chdir(),
            10 => self.sys_dup(),
            11 => self.sys_getpid(),
            12 => self.sys_sbrk(),
            13 => self.sys_sleep(),
            14 => self.sys_uptime(),
            15 => self.sys_open(),
            16 => self.sys_write(),
            17 => self.sys_mknod(),
            18 => self.sys_unlink(),
            19 => self.sys_link(),
            20 => self.sys_mkdir(),
            21 => self.sys_close(),
            _ => {
                panic!("unknown syscall num: {}", a7);
            }
        };
        tf.a0 = match sys_result {
            Ok(ret) => ret,
            Err(()) => -1isize as usize,
        };
    }

    /// Give up the current runing process in this cpu
    /// Change the name to yielding, because `yield` is a key word
    pub fn yielding(&mut self) {
        let mut guard = self.excl.lock();
        assert_eq!(guard.state, ProcState::RUNNING);
        guard.state = ProcState::RUNNABLE;
        guard = unsafe { CPU_MANAGER.my_cpu_mut().sched(guard,
            self.data.get_mut().get_context()) };
        drop(guard);
    }

    /// Atomically release a spinlock and sleep on chan.
    /// The passed-in guard should not the proc's guard,
    /// otherwise it will deadlock(because it acquires proc's lock first).
    /// Do not reacquires lock when awakened,
    /// so the caller must reacquire it if needed. 
    pub fn sleep<T>(&self, channel: usize, guard: SpinLockGuard<'_, T>) {
        // Must acquire p->lock in order to
        // change p->state and then call sched.
        // Once we hold p->lock, we can be
        // guaranteed that we won't miss any wakeup
        // (wakeup locks p->lock),
        // so it's okay to release lk.
        let mut excl_guard = self.excl.lock();
        drop(guard);

        // go to sleep
        excl_guard.channel = channel;
        excl_guard.state = ProcState::SLEEPING;

        unsafe {
            let c = CPU_MANAGER.my_cpu_mut();
            excl_guard = c.sched(excl_guard, 
                &mut (*self.data.get()).context as *mut _);
        }

        excl_guard.channel = 0;
        drop(excl_guard);
    }

    /// Fork a child process.
    fn fork(&mut self) -> Result<usize, ()> {
        let pdata = self.data.get_mut();
        let child = unsafe { PROC_MANAGER.alloc_proc().ok_or(())? };
        let mut cexcl = child.excl.lock();
        let cdata = unsafe { child.data.get().as_mut().unwrap() };

        // clone memory
        let cpgt = cdata.pagetable.as_mut().unwrap();
        let size = pdata.sz;
        if pdata.pagetable.as_mut().unwrap().uvm_copy(cpgt, size).is_err() {
            debug_assert_eq!(child.killed.load(Ordering::Relaxed), false);
            child.killed.store(false, Ordering::Relaxed);
            cdata.cleanup();
            cexcl.cleanup();
            return Err(())
        }
        cdata.sz = size;

        // clone trapframe and return 0 on a0
        unsafe {
            ptr::copy_nonoverlapping(pdata.tf, cdata.tf, 1);
            cdata.tf.as_mut().unwrap().a0 = 0;
        }

        // clone opened files and cwd
        cdata.open_files.clone_from(&pdata.open_files);
        cdata.cwd.clone_from(&pdata.cwd);
        
        // copy process name
        cdata.name.copy_from_slice(&pdata.name);

        let cpid = cexcl.pid;

        drop(cexcl);

        unsafe { PROC_MANAGER.set_parent(child.index, self.index); }

        let mut cexcl = child.excl.lock();
        cexcl.state = ProcState::RUNNABLE;
        drop(cexcl);

        Ok(cpid)
    }
}

impl Proc {
    /// Fetch register value.
    fn arg_raw(&self, n: usize) -> usize {
        let tf = unsafe { self.data.get().as_ref().unwrap().tf.as_ref().unwrap() };
        match n {
            0 => {tf.a0}
            1 => {tf.a1}
            2 => {tf.a2}
            3 => {tf.a3}
            4 => {tf.a4}
            5 => {tf.a5}
            _ => { panic!("n is larger than 5") }
        }
    }

    /// Fetch 32-bit register value.
    /// Note: `as` conversion is performed between usize and i32
    #[inline]
    fn arg_i32(&self, n: usize) -> i32 {
        self.arg_raw(n) as i32
    }

    /// Fetch a raw user virtual address from register value.
    /// Note: This raw address could be null,
    ///     and it might only be used to access user virtual address.
    #[inline]
    fn arg_addr(&self, n: usize) -> usize {
        self.arg_raw(n)
    }

    /// Fetch a file descriptor from register value.
    /// Also Check if the fd is valid.
    #[inline]
    fn arg_fd(&mut self, n: usize) -> Result<usize, ()> {
        let fd = self.arg_raw(n);
        if fd >= NFILE || self.data.get_mut().open_files[fd].is_none() {
            Err(())
        } else {
            Ok(fd)
        }
    }

    /// Fetch a null-terminated string from register pointer.
    fn arg_str(&self, n: usize, buf: &mut [u8]) -> Result<(), &'static str> {
        let addr: usize = self.arg_raw(n);
        let pagetable = unsafe { self.data.get().as_ref().unwrap().pagetable.as_ref().unwrap() };
        pagetable.copy_in_str(addr, buf)?;
        Ok(())
    }

    /// Fetch a virtual address at virtual address `addr`.
    fn fetch_addr(&self, addr: usize) -> Result<usize, &'static str> {
        let pd = unsafe { self.data.get().as_ref().unwrap() };
        if addr + mem::size_of::<usize>() > pd.sz {
            Err("input addr > proc's mem size")
        } else {
            let mut ret: usize = 0;
            match pd.copy_in(
                addr, 
                &mut ret as *mut usize as *mut u8, 
                mem::size_of::<usize>()
            ) {
                Ok(_) => Ok(ret),
                Err(_) => Err("pagetable copy_in eror"),
            }
        }
    }

    /// Fetch a null-nullterminated string from virtual address `addr` into the kernel buffer.
    fn fetch_str(&self, addr: usize, dst: &mut [u8]) -> Result<(), &'static str>{
        let pd = unsafe { self.data.get().as_ref().unwrap() };
        pd.pagetable.as_ref().unwrap().copy_in_str(addr, dst)
    }
}

/// first user program that calls exec("/init")
static INITCODE: [u8; 51] = [
    0x17, 0x05, 0x00, 0x00, 0x13, 0x05, 0x05, 0x02, 0x97, 0x05, 0x00, 0x00, 0x93, 0x85, 0x05, 0x02,
    0x9d, 0x48, 0x73, 0x00, 0x00, 0x00, 0x89, 0x48, 0x73, 0x00, 0x00, 0x00, 0xef, 0xf0, 0xbf, 0xff,
    0x2f, 0x69, 0x6e, 0x69, 0x74, 0x00, 0x00, 0x01, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00,
];
