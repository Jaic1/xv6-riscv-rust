use array_macro::array;

use core::convert::TryFrom;
use core::ptr;

use crate::consts::{NPROC, PGSIZE, TRAMPOLINE};
use crate::mm::{kalloc, kvm_map, PhysAddr, PteFlag, VirtAddr};
use crate::spinlock::SpinLock;
use crate::trap::user_trap_ret;
use crate::fs::{self, ROOTDEV};

pub use cpu::{CPU_MANAGER, CpuManager};
pub use cpu::{push_off, pop_off};

mod context;
mod proc;
mod cpu;
mod trapframe;
mod syscall;
mod elf;

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
            let pa = kalloc().expect("no enough page for proc's kstack");
            let va = kstack(pos);
            kvm_map(
                VirtAddr::try_from(va).unwrap(),
                PhysAddr::try_from(pa as usize).unwrap(),
                PGSIZE,
                PteFlag::R | PteFlag::W,
            );
            p.data.get_mut().set_kstack(pa as usize);
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
                    match unsafe { kalloc() } {
                        Some(ptr) => {
                            pd.set_tf(ptr as *mut TrapFrame);
                        },
                        None => {
                            drop(guard);
                            return None
                        },
                    }

                    pd.proc_pagetable();
                    pd.init_context();
                    guard.pid = new_pid;
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

    /// Set up first process
    /// Only called once by the initial hart
    /// which can guarantee the init proc's index at table is 0
    pub unsafe fn user_init(&mut self) {
        let p = self.alloc_proc()
            .expect("user_init: all process should be unused");
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
/// 
/// Need to be handled carefully, because CPU use ra to jump here
unsafe fn fork_ret() -> ! {
    static mut FIRST: bool = true;
    
    // Still holding p->lock from scheduler
    CPU_MANAGER.my_proc().excl.unlock();
    
    if FIRST {
        // File system initialization
        FIRST = false;
        fs::init(ROOTDEV);
    }

    user_trap_ret();
}

#[inline]
fn kstack(pos: usize) -> usize {
    Into::<usize>::into(TRAMPOLINE) - (pos + 1) * 2 * PGSIZE
}
