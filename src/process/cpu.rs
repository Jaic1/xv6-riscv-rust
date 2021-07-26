use array_macro::array;

use core::ptr;

use crate::register::{tp, sstatus};
use crate::spinlock::SpinLockGuard;
use crate::consts::NCPU;
use super::{Context, PROC_MANAGER, Proc, ProcState, proc::ProcExcl};

pub static mut CPU_MANAGER: CpuManager = CpuManager::new();

pub struct CpuManager {
    table: [Cpu; NCPU]
}

impl CpuManager {
    const fn new() -> Self {
        Self {
            table: array![_ => Cpu::new(); NCPU],
        }
    }

    /// Must be called with interrupts disabled,
    /// to prevent race with process being moved
    /// to a different CPU.
    #[inline]
    pub unsafe fn cpu_id() -> usize {
        tp::read()
    }

    /// Return the reference this CPU's cpu struct.
    /// Interrupts must be disabled.
    unsafe fn my_cpu(&self) -> &Cpu {
        let id = Self::cpu_id();
        &self.table[id]
    }

    /// Return the mutable reference this CPU's cpu struct.
    /// Interrupts must be disabled.
    pub unsafe fn my_cpu_mut(&mut self) -> &mut Cpu {
        let id = Self::cpu_id();
        &mut self.table[id]
    }

    /// Get the running process.
    /// Can be called on different cpu simultaneously
    /// If no process running, calling this method will panic
    /// TODO - Is giving out raw pointer better?
    pub fn my_proc(&self) -> &mut Proc {
        let p;
        push_off();
        unsafe {
            let c = self.my_cpu();
            if c.proc.is_null() {
                panic!("my_proc(): no process running");
            }
            p = &mut *c.proc;
        }
        pop_off();
        p
    }

    /// Scheduler loop, never return
    /// jumped from rust_main in rmain.rs
    /// called simultaneously by different harts
    pub unsafe fn scheduler(&mut self) -> ! {
        extern "C" {
            fn swtch(old: *mut Context, new: *mut Context);
        }

        let c = self.my_cpu_mut();

        loop {
            // ensure devices can interrupt
            sstatus::intr_on();

            // use ProcManager to find a runnable process
            match PROC_MANAGER.alloc_runnable() {
                Some(p) => {
                    c.proc = p as *mut _;
                    let mut guard = p.excl.lock();
                    guard.state = ProcState::RUNNING;

                    swtch(&mut c.scheduler as *mut Context,
                        p.data.get_mut().get_context());
                    
                    if c.proc.is_null() {
                        panic!("context switch back with no process reference");
                    }
                    c.proc = ptr::null_mut();
                    drop(guard);
                },
                None => {},
            }
        }
    }
}

/// Cpu contains current info about the running cpu 
///
/// no need to bind a spinlock to it,
/// since only one hart will use this struct
pub struct Cpu {
    proc: *mut Proc,
    scheduler: Context,
    noff: u8,
    intena: bool,
}

impl Cpu {
    const fn new() -> Self {
        Self {
            proc: ptr::null_mut(),
            scheduler: Context::new(),
            noff: 0,
            intena: false,
        }
    }

    /// Switch back to scheduler.
    /// Passing in and out a guard,
    /// beacuse we need to hold the proc lock during this method.
    pub unsafe fn sched<'a>(&mut self, guard: SpinLockGuard<'a, ProcExcl>, ctx: *mut Context)
        -> SpinLockGuard<'a, ProcExcl>
    {
        extern "C" {
            fn swtch(old: *mut Context, new: *mut Context);
        }

        // interrupt is off
        if !guard.holding() {
            panic!("sched(): not holding proc's lock");
        }
        // only holding self.proc's lock
        if self.noff != 1 {
            panic!("sched(): cpu hold multi locks");
        }
        // proc is not running
        if guard.state == ProcState::RUNNING {
            panic!("sched(): proc is running");
        }
        // should not be interruptible
        if sstatus::intr_get() {
            panic!("sched(): interruptible");
        }

        let intena = self.intena;
        swtch(ctx, &mut self.scheduler as *mut Context);
        self.intena = intena;

        guard
    }

    /// Yield the holding process if any and it's RUNNING.
    /// Directly return if none.
    pub fn try_yield_proc(&mut self) {
        if !self.proc.is_null() {
            let guard = unsafe {
                self.proc.as_mut().unwrap().excl.lock()
            };
            if guard.state == ProcState::RUNNING {
                drop(guard);
                unsafe { self.proc.as_mut().unwrap().yielding(); }
            } else {
                drop(guard);
            }
        }
    }
}

/// push_off/pop_off are like intr_off()/intr_on() except that they are matched:
/// it takes two pop_off()s to undo two push_off()s.  Also, if interrupts
/// are initially off, then push_off, pop_off leaves them off.
pub fn push_off() {
    let old = sstatus::intr_get();
    sstatus::intr_off();
    let c = unsafe { CPU_MANAGER.my_cpu_mut() };
    if c.noff == 0 {
        c.intena = old;
    }
    c.noff += 1;
}

pub fn pop_off() {
    if sstatus::intr_get() {
        panic!("pop_off(): interruptable");
    }
    let c = unsafe { CPU_MANAGER.my_cpu_mut() };
    if c.noff.checked_sub(1).is_none() {
        panic!("pop_off(): count not match");
    }
    c.noff -= 1;
    if c.noff == 0 && c.intena {
        sstatus::intr_on();
    }
}
