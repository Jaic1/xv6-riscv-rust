use core::convert::TryFrom;

use crate::consts::{NPROC, PGSIZE, TRAMPOLINE};
use crate::mm::{kalloc, kvm_map, PhysAddr, PteFlag, VirtAddr};
use crate::spinlock::SpinLock;

pub use cpu::{cpu_id, push_off, pop_off, my_cpu};

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
            table: [Proc::new(); NPROC],
            init_proc: 0,
            pid: SpinLock::new(0, "nextpid"),
        }
    }

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
            p.set_kstack(pa as usize);
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
    /// and return with p->lock held.
    /// If there are no free procs, return 0.
    fn alloc_proc(&mut self) ->
        Option<&mut Proc>
    {
        for i in 0..self.table.len() {
            let p = &mut self.table[i];
            unsafe {p.lock.acquire_lock();}
            match p.state {
                ProcState::UNUSED => {
                    //p.pid = self.alloc_pid();
                    match unsafe { kalloc() } {
                        Some(ptr) => {
                            p.set_tf(ptr as *mut TrapFrame);
                        },
                        None => {
                            unsafe {p.lock.release_lock();}
                            return None
                        },
                    }
                    p.proc_pagetable();
                    p.init_context();
                    return Some(&mut self.table[i])
                },
                _ => {},
            }
            unsafe {p.lock.release_lock();}
        }

        None
    }

    /// Look in the process table for an RUNNABLE proc.
    /// Typically used in each cpu's scheduler
    fn alloc_runnable(&mut self) ->
        Option<&mut Proc>
    {
        for i in 0..self.table.len() {
            unsafe {self.table[i].lock.acquire_lock();}
            match self.table[i].state {
                ProcState::RUNNABLE => {
                    return Some(&mut self.table[i])
                },
                _ => {},
            }
            unsafe {self.table[i].lock.release_lock();}
        }

        None
    }

    /// Set up first process
    /// Only called once in rust_main(),
    /// which can guarantee the init proc's index at table is 0
    pub unsafe fn user_init(&mut self) {
        let p = self.alloc_proc().expect("user_init: all process should be unused");
        p.user_init();
        p.lock.release_lock();
    }
}

/// A fork child's very first scheduling by scheduler()
/// will swtch to forkret.
/// 
/// Need to be handled carefully, because CPU use ra to jump here
unsafe fn fork_ret() -> ! {
    static mut FIRST: bool = true;
    
    // Still holding p->lock from scheduler
    // TODO
    
    if FIRST {
        // File system initialization
        // TODO
        FIRST = false;
        // fsinit()
    }

    // TODO
    // user_trap_ret();
    panic!("in fork_ret");
}

#[inline]
fn kstack(pos: usize) -> usize {
    Into::<usize>::into(TRAMPOLINE) - (pos + 1) * 2 * PGSIZE
}
