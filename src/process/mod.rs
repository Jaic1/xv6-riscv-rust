use array_macro::array;

use core::convert::TryFrom;
use core::ptr;

use crate::consts::{NPROC, PGSIZE, TRAMPOLINE, fs::ROOTDEV};
use crate::mm::{kvm_map, PhysAddr, PteFlag, VirtAddr, RawPage, RawSinglePage, PageTable, RawQuadPage};
use crate::spinlock::SpinLock;
use crate::trap::user_trap_ret;
use crate::fs;

pub use cpu::{CPU_MANAGER, CpuManager};
pub use cpu::{push_off, pop_off};

mod context;
mod proc;
mod cpu;
mod trapframe;

use context::Context;
use proc::{Proc, ProcState};
use trapframe::TrapFrame;

// no lock to protect PROC_MANAGER, i.e.,
// no lock to protect the whole process table
// 
// Accessing it in other places is unsafe, which is not satisfying.
// may subject to change
pub static mut PROC_MANAGER: ProcManager = ProcManager::new();

pub struct ProcManager {
    table: [Proc; NPROC],
    init_proc: usize,
    pid: SpinLock<usize>,
}

impl ProcManager {
    const fn new() -> Self {
        Self {
            table: array![_ => Proc::new(); NPROC],
            init_proc: 0,
            pid: SpinLock::new(0, "nextpid"),
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
    /// and return with its ProcExcl held.
    /// If there are no free procs, return None.
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
                    pd.tf = unsafe { RawSinglePage::new_zeroed() as *mut TrapFrame };

                    debug_assert!(pd.pagetable.is_none());
                    pd.pagetable = Some(PageTable::alloc_proc_pagetable(pd.tf as usize));
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
