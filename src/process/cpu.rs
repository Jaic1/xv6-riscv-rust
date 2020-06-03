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
fn intr_on() {
    sie::intr_on();
    sstatus::intr_on();
}
