//! spinlock module
//! unlike xv6-riscv, xv6-riscv-rust wraps data into a spinlock
//! useful reference crate spin(https://crates.io/crates/spin)

use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::{UnsafeCell, Cell};
use crate::proc;
use crate::register::sstatus;

pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,

    /// for debugging
    /// None means this spinlock is not held by any cpu
    /// TODO - Cell vs UnsafeCell
    cpu_id: Cell<Option<usize>>,
    name: &'static str,
}

pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a AtomicBool,
    data: &'a mut T,
}

impl<T> SpinLock<T> {
    pub const fn new(user_data: T, name: &str) -> SpinLock<T> {
        SpinLock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(user_data),
            cpu_id: Cell::new(None),
            name,
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    fn holding(&self) -> bool {
        let r: bool;
        push_off();
        unsafe {
            r = self.lock.load(Ordering::Relaxed) &&
                self.cpuid.into_inner() == Some(proc::cpu_id());
        }
        pop_off();
        r
    }

    fn obtain_lock(&self) {
        push_off();
        if self.holding() {
            panic!("acquire");
        }
        while self.lock.compare_and_swap(false, true, Ordering::Acquire) != false {}
        // TODO - __sync_synchronize
        unsafe { self.cpuid.set(Some(proc::cpu_id())); }
    }
}

/// push_off/pop_off are like intr_off()/intr_on() except that they are matched:
/// it takes two pop_off()s to undo two push_off()s.  Also, if interrupts
/// are initially off, then push_off, pop_off leaves them off.
fn push_off() {
    let old: bool = sstatus::intr_get();
    sstatus::intr_off();
    proc::push_off(old);
}

fn pop_off() {
    if sstatus::intr_get() {
        panic!("spinlock.rs: pop_off - interruptable");
    }
    // a little difference from xv6-riscv
    // optional intr_on() moved to proc::pop_off()
    proc::pop_off();
}














