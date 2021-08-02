use array_macro::array;

use core::convert::TryFrom;
use core::ptr;
use core::mem;
use core::sync::atomic::Ordering;

use crate::consts::{NPROC, PGSIZE, TRAMPOLINE, fs::ROOTDEV};
use crate::mm::{kvm_map, PhysAddr, PteFlag, VirtAddr, RawPage, RawSinglePage, PageTable, RawQuadPage};
use crate::spinlock::SpinLock;
use crate::trap::user_trap_ret;
use crate::fs;

pub use cpu::{CPU_MANAGER, CpuManager};
pub use cpu::{push_off, pop_off};
pub use proc::Proc;

mod context;
mod proc;
mod cpu;
mod trapframe;

use context::Context;
use proc::ProcState;
use trapframe::TrapFrame;

// no lock to protect PROC_MANAGER, i.e.,
// no lock to protect the whole process table
// 
// Accessing it in other places is unsafe, which is not satisfying.
// may subject to change
pub static mut PROC_MANAGER: ProcManager = ProcManager::new();

pub struct ProcManager {
    table: [Proc; NPROC],
    parents: SpinLock<[Option<usize>; NPROC]>,
    init_proc: usize,
    pid: SpinLock<usize>,
}

impl ProcManager {
    const fn new() -> Self {
        Self {
            table: array![i => Proc::new(i); NPROC],
            parents: SpinLock::new(array![_ => None; NPROC], "proc parents"),
            init_proc: 0,
            pid: SpinLock::new(0, "pid"),
        }
    }

    /// Only called once by the initial hart
    pub unsafe fn proc_init(&mut self) {
        for (pos, p) in self.table.iter_mut().enumerate() {
            // Allocate a page for the process's kernel stack.
            // Map it high in memory, followed by an invalid
            // guard page.
            let pa = RawQuadPage::new_zeroed() as usize;
            let va = kstack(pos);
            kvm_map(
                VirtAddr::try_from(va).unwrap(),
                PhysAddr::try_from(pa).unwrap(),
                PGSIZE*4,
                PteFlag::R | PteFlag::W,
            );
            p.data.get_mut().set_kstack(va);
        }
    }

    /// Allocate pid
    /// It can be accessed simultaneously
    fn alloc_pid(&self) -> usize {
        let ret_pid: usize;
        let mut pid = self.pid.lock();
        ret_pid = *pid;
        *pid += 1;
        drop(pid);
        ret_pid
    }

    /// Look in the process table for an UNUSED proc.
    /// If found, initialize state required to run in the kernel,
    /// and return without its ProcExcl held.
    /// Note: The returned [`Proc`] is in [`ProcState::ALLOCATED`].
    /// LTODO - Should recover from OOM?
    fn alloc_proc(&mut self) ->
        Option<&mut Proc>
    {
        let new_pid = self.alloc_pid();

        for p in self.table.iter_mut() {
            let mut guard = p.excl.lock();
            match guard.state {
                ProcState::UNUSED => {
                    // holding the process's excl lock,
                    // so manager can modify its private data
                    let pd = p.data.get_mut();

                    // alloc trapframe
                    pd.tf = unsafe { RawSinglePage::try_new_zeroed().ok()? as *mut TrapFrame };

                    debug_assert!(pd.pagetable.is_none());
                    match PageTable::alloc_proc_pagetable(pd.tf as usize) {
                        Some(pgt) => pd.pagetable = Some(pgt),
                        None => {
                            unsafe { RawSinglePage::from_raw_and_drop(pd.tf as *mut u8); }
                            return None
                        },
                    }
                    pd.init_context();
                    guard.pid = new_pid;
                    guard.state = ProcState::ALLOCATED;

                    drop(guard);
                    return Some(p)
                },
                _ => drop(guard),
            }
        }

        None
    }

    /// Look in the process table for an RUNNABLE proc,
    /// set its state to ALLOCATED and return without the proc's lock held.
    /// Typically used in each cpu's scheduler
    fn alloc_runnable(&mut self) ->
        Option<&mut Proc>
    {
        for p in self.table.iter_mut() {
            let mut guard = p.excl.lock();
            match guard.state {
                ProcState::RUNNABLE => {
                    guard.state = ProcState::ALLOCATED;
                    drop(guard);
                    return Some(p)
                },
                _ => {
                    drop(guard);
                },
            }
        }

        None
    }

    /// Set up first process.
    /// SAFETY: Only called once by the initial hart,
    /// which can guarantee the init proc's index at table is 0.
    pub unsafe fn user_init(&mut self) {
        let p = self.alloc_proc()
            .expect("all process should be unused");
        p.user_init();
        let mut guard = p.excl.lock();
        guard.state = ProcState::RUNNABLE;
    }

    /// Check if the given process is the init_proc 
    fn is_init_proc(&self, p: &Proc) -> bool {
        ptr::eq(&self.table[0], p)
    }

    /// Wake up all processes sleeping on chan.
    /// Must be called without any p->lock.
    pub fn wakeup(&self, channel: usize) {
        for p in self.table.iter() {
            let mut guard = p.excl.lock();
            if guard.state == ProcState::SLEEPING && guard.channel == channel {
                guard.state = ProcState::RUNNABLE;
            }
            drop(guard);
        }
    }

    /// Set a newly created process's parent.
    fn set_parent(&self, child_i: usize, parent_i: usize) {
        let mut guard = self.parents.lock();
        let ret = guard[child_i].replace(parent_i);
        debug_assert!(ret.is_none());
        drop(guard);
    }

    /// Put a process to exit, does not return.
    fn exiting(&self, exit_pi: usize, exit_status: i32) {
        if exit_pi == self.init_proc {
            panic!("init process exiting");
        }

        unsafe { self.table[exit_pi].data.get().as_mut().unwrap().close_files(); }

        let mut parent_map = self.parents.lock();

        // Set the children's parent to init process.
        let mut have_child = false;
        for child in parent_map.iter_mut() {
            match child {
                Some(parent) if *parent == exit_pi => {
                    *parent = self.init_proc;
                    have_child = true;
                },
                _ => {},
            }
        }
        if have_child {
            self.wakeup(&self.table[self.init_proc] as *const Proc as usize);
        }
        let exit_parenti = *parent_map[exit_pi].as_ref().unwrap();
        self.wakeup(&self.table[exit_parenti] as *const Proc as usize);

        let mut exit_pexcl = self.table[exit_pi].excl.lock();
        exit_pexcl.exit_status = exit_status;
        exit_pexcl.state = ProcState::ZOMBIE;
        drop(parent_map);
        unsafe {
            let exit_ctx = self.table[exit_pi].data.get().as_mut().unwrap().get_context();
            CPU_MANAGER.my_cpu_mut().sched(exit_pexcl, exit_ctx);
        }

        unreachable!("exiting {}", exit_pi);
    }

    /// Wait for a child process to exit/ZOMBIE.
    /// Return the child's pid if any, return `Err(())` if none. 
    fn waiting(&self, pi: usize, addr: usize) -> Result<usize, ()> {
        let mut parent_map = self.parents.lock();
        let p = unsafe { CPU_MANAGER.my_proc() };
        let pdata = unsafe { p.data.get().as_mut().unwrap() };

        loop {
            let mut have_child = false;
            for i in 0..NPROC {
                if parent_map[i].is_none() || *parent_map[i].as_ref().unwrap() != pi {
                    continue;
                }

                let mut child_excl = self.table[i].excl.lock();
                have_child = true;
                if child_excl.state != ProcState::ZOMBIE {
                    continue;
                }
                let child_pid = child_excl.pid;
                if addr != 0 && pdata.copy_out(&child_excl.exit_status as *const _ as *const u8,
                    addr, mem::size_of_val(&child_excl.exit_status)).is_err()
                {
                    return Err(())
                }
                parent_map[i].take();
                self.table[i].killed.store(false, Ordering::Relaxed);
                let child_data = unsafe { self.table[i].data.get().as_mut().unwrap() };
                child_data.cleanup();
                child_excl.cleanup();           
                return Ok(child_pid)
            }

            if !have_child || p.killed.load(Ordering::Relaxed) {
                return Err(())
            }

            // have children, but none of them exit
            let channel = p as *const Proc as usize;
            p.sleep(channel, parent_map);
            parent_map = self.parents.lock();
        }
    }

    /// Kill a process with given pid.
    pub fn kill(&self, pid: usize) -> Result<(), ()> {
        for i in 0..NPROC {
            let mut guard = self.table[i].excl.lock();
            if guard.pid == pid {
                self.table[i].killed.store(true, Ordering::Relaxed);
                if guard.state == ProcState::SLEEPING {
                    guard.state = ProcState::RUNNABLE;
                }
                return Ok(())
            }
        }

        Err(())
    }
}

/// A fork child's very first scheduling by scheduler()
/// will swtch to forkret.
/// Need to be handled carefully, because CPU use ra to jump here
/// 
/// SAFERT1: It should be called by the first user process alone,
///         and then other user processes can call fork_ret concurrently.
/// SAFETY2: It is an non-reentrant function.
///         Interrupt/Exception handler must not call this function.
unsafe fn fork_ret() -> ! {
    static mut INITIALIZED: bool = false;
    
    // Still holding p->lock from scheduler
    CPU_MANAGER.my_proc().excl.unlock();
    
    if !INITIALIZED {
        INITIALIZED = true;
        // File system initialization
        fs::init(ROOTDEV);
    }

    user_trap_ret();
}

#[inline]
fn kstack(pos: usize) -> usize {
    Into::<usize>::into(TRAMPOLINE) - (pos + 1) * 5 * PGSIZE
}
