use crate::consts::NCPU;
use crate::register::{sie, sstatus, tp};

static mut CPUS: [Cpu; NCPU] = [Cpu::new(); NCPU];

/// Must be called with interrupts disabled,
/// to prevent race with process being moved
/// to a different CPU.
pub unsafe fn cpu_id() -> usize {
    tp::read()
}

// /// Return this CPU's cpu struct.
// /// Interrupts must be disabled.
// unsafe fn my_cpu() -> &mut Cpu {
//     let id = cpu_id();
//     &mut CPUS[id]
// }

/// Cpu contains current info about this hart
///
/// no need to bind a spinlock to it,
/// since only one hart will use this struct
///
struct Cpu {
    noff: u8,
    intena: bool,
}

impl Cpu {
    const fn new() -> Cpu {
        Cpu {
            noff: 0,
            intena: false,
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
        panic!("proc.rs: pop_off");
    }
    c.noff -= 1;
    if c.noff == 0 && c.intena {
        sie::intr_on();
        sstatus::intr_on();
    }
}
