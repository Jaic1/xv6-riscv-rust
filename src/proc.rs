use core::convert::TryFrom;

use crate::consts::{NCPU, NPROC, PGSIZE, TRAMPOLINE};
use crate::mm::{kalloc, kvm_map, PhysAddr, PteFlag, VirtAddr};
use crate::spinlock::SpinLock;
use crate::register::{sie, sstatus, tp};

static mut PROCS: [Proc; NPROC] = [Proc::new(); NPROC];

enum ProcState { UNUSED, SLEEPING, RUNNABLE, RUNNING, ZOMBIE }

pub struct Proc {
    lock: SpinLock<()>,

    // p->lock must be held when using these:
    state: ProcState,

    // lock need not be held
    kstack: usize,
}

impl Proc {
    const fn new() -> Self {
        Self {
            lock: SpinLock::new((), "proc"),

            state: ProcState::UNUSED,

            kstack: 0,
        }
    }

    const fn kstack(pos: usize) -> usize {
        TRAMPOLINE - (pos + 1) * 2 * PGSIZE
    }
}

pub unsafe fn proc_init() {
    for (pos, p) in PROCS.iter_mut().enumerate() {
        // Allocate a page for the process's kernel stack.
        // Map it high in memory, followed by an invalid
        // guard page.
        let pa = kalloc().expect("no enough page for proc's kstack");
        let va = Proc::kstack(pos);
        kvm_map(
            VirtAddr::try_from(va).unwrap(),
            PhysAddr::try_from(pa as usize).unwrap(),
            PGSIZE,
            PteFlag::R | PteFlag::W,
        );
        p.kstack = pa as usize;
    }
}

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
    noff: u8,
    intena: bool,
}

impl<'a> Cpu<'a> {
    const fn new() -> Self {
        Self {
            proc: None,
            noff: 0,
            intena: false,
        }
    }

    // rmain jumps here with my_cpu()
    pub unsafe fn scheduler(&mut self) -> ! {
        extern "C" {
            fn swtch();
        }

        loop {
            // ensure devices can interrupt
            intr_on();

            for i in 0..NPROC {
                let _lock = PROCS[i].lock.lock();

                match PROCS[i].state {
                    ProcState::RUNNABLE => {
                        // Switch to chosen process.  It is the process's job
                        // to release its lock and then reacquire it
                        // before jumping back to us.
                        PROCS[i].state = ProcState::RUNNING;
                        self.proc = Some(&mut PROCS[i]);

                        swtch();    // TODO

                        // Process is done running for now.
                        // It should have changed its p->state before coming back.
                        self.proc = None;
                    }
                    _ => {}
                }

                drop(_lock);
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

// enable device interrupts
fn intr_on() {
    sie::intr_on();
    sstatus::intr_on();
}
