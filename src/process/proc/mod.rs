use alloc::boxed::Box;
use core::mem;
use core::option::Option;
use core::ptr;
use core::cell::UnsafeCell;

use crate::consts::{PGSIZE, fs::ROOTIPATH};
use crate::mm::PageTable;
use crate::register::{satp, sepc, sstatus};
use crate::spinlock::{SpinLock, SpinLockGuard};
use crate::trap::user_trap;
use crate::fs::{Inode, ICACHE};

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
    pub channel: usize,
    pub pid: usize,
}

impl ProcExcl {
    const fn new() -> Self {
        Self {
            state: ProcState::UNUSED,
            channel: 0,
            pid: 0,
        }
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
            pagetable: None,
            tf: ptr::null_mut(),
            context: Context::new(),
            name: [0; 16],
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
}

/// Process Struct
/// 
/// ProcData could be protected by RefCell,
/// but in case when the process is mutating the ProcData,
/// but then if it is interrupted and get killed, so it need to
/// clean its ProcData, so UnsafeCell is better.
pub struct Proc {
    pub excl: SpinLock<ProcExcl>,
    pub data: UnsafeCell<ProcData>,
    killed: bool,
}

impl Proc {
    pub const fn new() -> Self {
        Self {
            excl: SpinLock::new(ProcExcl::new(), "ProcExcl"),
            data: UnsafeCell::new(ProcData::new()),
            killed: false,
        }
    }

    /// Called by ProcManager's user_init,
    /// Only be called once for the first user process
    /// TODO - copy user code and sth else
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

    /// Exit the current process. No return.
    /// Remain zombie state until its parent recycle its resource,
    /// like trapframe and pagetable.
    pub fn exit(&mut self, status: isize) {
        if unsafe { PROC_MANAGER.is_init_proc(&self) } {
            panic!("init_proc exiting");
        }

        todo!("exit: status={}", status);
    }

    /// Abondon current process if
    /// the killed flag is true
    pub fn check_abondon(&mut self, status: isize) {
        if self.killed {
            self.exit(status);
        }
    }

    /// Abondon current process by:
    /// 1. setting its killed flag to true
    /// 2. and then exit
    pub fn abondon(&mut self, status: isize) {
        self.killed = true;
        self.exit(status);
    }

    /// Handle system call
    /// It may be interrrupted in the procedure of syscall
    pub fn syscall(&mut self) {
        sstatus::intr_on();

        let tf = unsafe { self.data.get_mut().tf.as_mut().unwrap() };
        let a7 = tf.a7;
        tf.admit_ecall();
        let sys_result = match a7 {
            7 => self.sys_exec(),
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
            &mut self.data.get_mut().context as *mut _) };
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
