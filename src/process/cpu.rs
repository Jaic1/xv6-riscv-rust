use crate::register::{tp, sie, sstatus};
use crate::consts::NCPU;

use super::{Context, Proc, PROC_MANAGER, ProcState};

static mut CPUS: [Cpu; NCPU] = [Cpu::new(); NCPU];

/// Must be called with interrupts disabled,
/// to prevent race with process being moved
/// to a different CPU.
pub unsafe fn cpu_id() -> usize {
    tp::read()
}

/// Return this CPU's cpu struct.
/// Interrupts must be disabled.
pub unsafe fn my_cpu() -> &'static mut Cpu<'static> {
    let id = cpu_id();
    &mut CPUS[id]
}

/// Cpu contains current info about this hart
///
/// no need to bind a spinlock to it,
/// since only one hart will use this struct
///
pub struct Cpu<'a> {
    proc: Option<&'a mut Proc>,
    scheduler: Context,
    noff: u8,
    intena: bool,
}

impl<'a> Cpu<'a> {
    const fn new() -> Self {
        Self {
            proc: None,
            scheduler: Context::new(),
            noff: 0,
            intena: false,
        }
    }

    /// rmain jumps here with my_cpu()
    pub unsafe fn scheduler(&mut self) -> ! {
        extern "C" {
            fn swtch(old: *mut Context, new: *mut Context);
        }

        loop {
            // ensure devices can interrupt
            intr_on();

            // use ProcManager to find a runnable process
            match PROC_MANAGER.alloc_runnable() {
                Some(p) => {
                    p.state = ProcState::RUNNING;
                    self.proc = Some(p);

                    swtch(&mut self.scheduler as *mut Context,
                        self.proc
                            .as_mut()
                            .unwrap()
                            .get_context_mut()
                            as *mut Context);
                    
                    let p = self.proc
                        .take()
                        .expect("context switch back with no process reference");
                    p.lock.release_lock();
                },
                None => {},
            }
        }
    }

    /// Switch back to scheduler.
    /// see more in xv6-riscv
    unsafe fn sched(&mut self) {
        extern "C" {
            fn swtch(old: *mut Context, new: *mut Context);
        }

        // should not be interruptible
        if sstatus::intr_get() {
            panic!("sched: interruptible");
        }

        // only holding self.proc's lock
        if self.noff != 1 {
            panic!("sched: locks")
        }

        // not using match
        // because that will move the mut reference out
        if self.proc.is_none() {
            panic!("sched: cpu {} have no proc reference", cpu_id());
        } else {
            let p = self.proc.as_mut().unwrap();
            if !p.lock.holding() {
                panic!("sched: not holding proc's lock");
            }
            if p.state == ProcState::RUNNING {
                panic!("sched: current proc is still running");
            }

            let intena = self.intena;
            swtch(p.get_context_mut() as *mut Context,
                &mut self.scheduler as *mut Context);
            self.intena = intena;
        }
    }

    /// Give up the current runing process in this cpu
    /// Interrupt should be off
    /// The referenced process's state should be running
    /// Change the name to yielding, because `yield` is a key word
    pub fn yielding(&mut self) {
        // not using match
        // because that will move the mut reference out
        // ignore none case in case the cpu is scheduling
        if self.proc.is_some() {
            // note: p is the copy of &'a mut Proc
            //      and self.proc may refer others in the middle
            let p = self.proc.as_mut().unwrap();
            unsafe {p.lock.acquire_lock();}
            assert_eq!(p.state, ProcState::RUNNING);
            p.state = ProcState::RUNNABLE;
            unsafe {self.sched();}
            let p = self.proc.as_mut().unwrap();
            unsafe {p.lock.release_lock();}
        }
    }

    /// Release the process's lock
    /// Only used in fork_ret or
    /// places not having current cpu's reference
    pub unsafe fn release_proc(&self) {
        self.proc.as_ref().unwrap().lock.release_lock();
    }

    /// Prepare for the user trap return
    /// Return current proc's satp for assembly code to switch page table
    pub fn user_ret_prepare(&mut self) -> usize {
        if self.proc.is_none() {
            panic!("Cpu's user_ret_prepare: holding no process");
        } else {
            self.proc.as_mut().unwrap().user_ret_prepare()
        }
    }

    /// Try to abondon current process if its killed flag is set
    /// No need to clear self.proc reference, scheduler thread will do it
    pub fn try_abondon(&mut self, status: isize) {
        if self.proc.as_ref().unwrap().killed {
            self.proc.as_mut().unwrap().exit(status);
        }
    }

    /// Abondon current process by setting its killed flag to true
    pub fn abondon(&mut self, status: isize) {
        let p = self.proc.as_mut().unwrap();
        p.killed = true;
        p.exit(status);
    }

    /// Handle syscall from user code, typically by ecall
    /// handle_trap jumps here
    pub fn syscall(&mut self) {
        self.try_abondon(-1);
        self.proc.as_mut().unwrap().syscall();
        self.try_abondon(-1);
    }
}

/// Called in spinlock's push_off().
/// Interrupts must be disabled due to its use of mut ref to CPUS.
pub fn push_off(old: bool) {
    let c;
    unsafe {
        c = &mut CPUS[cpu_id()];
    }
    if c.noff == 0 {
        c.intena = old;
    }
    c.noff += 1;
}

/// Called in spinlock's pop_off().
/// Interrupts must be disabled due to its use of mut ref to CPUS.
pub fn pop_off() {
    let c;
    unsafe {
        c = &mut CPUS[cpu_id()];
    }
    if c.noff.checked_sub(1).is_none() {
        panic!("cpu: pop_off");
    }
    c.noff -= 1;
    if c.noff == 0 && c.intena {
        intr_on();
    }
}

/// enable device interrupts
#[inline]
pub fn intr_on() {
    sie::intr_on();
    sstatus::intr_on();
}
