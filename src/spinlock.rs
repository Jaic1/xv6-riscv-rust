//! spinlock module
//! A spinlock wraps data into itself to protect them

use core::cell::{Cell, UnsafeCell};
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{fence, AtomicBool, Ordering};
use core::ptr::addr_of_mut;

use crate::process::{CpuManager, pop_off, push_off};

#[derive(Debug)]
pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    name: &'static str,
    cpuid: Cell<isize>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
// This is not needed for xv6-riscv-rust's spinlock?, while this is implemented both in crate std and spin.
// unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            lock: AtomicBool::new(false),
            name,
            cpuid: Cell::new(-1),
            data: UnsafeCell::new(data),
        }
    }

    /// Init the name of the [`SpinLock`].
    /// Useful when the memory is allocated but not initialized.
    /// SAFETY: This should be called when there is only one thread owns this [`SpinLock`].
    #[inline(always)]
    pub unsafe fn init_name(lock: *mut Self, name: &'static str) {
        addr_of_mut!((*lock).name).write(name);
    }
}

impl<T: ?Sized> SpinLock<T> {
    /// Locks the spinlock and returns a guard.
    ///
    /// The returned guard can be deferenced for data access.
    /// i.e., we implement Deref trait for the guard.
    /// Also, the lock will also be dropped when the guard falls out of scope.
    ///
    /// ```
    /// let proc = SpinLock::new(0);
    /// {
    ///     let mut proc_locked = proc.lock();
    ///     // The lock is now locked and the data can be accessed
    ///     *proc_locked = 1;
    ///     // The lock is going to fall out of scope
    ///     // i.e. the lock will be released
    /// }
    /// ```
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        self.acquire();
        SpinLockGuard {
            lock: &self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    /// Check whether this cpu is holding the lock.
    /// Interrupts must be off,
    /// because it call cpu_id()
    unsafe fn holding(&self) -> bool {
        self.lock.load(Ordering::Relaxed) && (self.cpuid.get() == CpuManager::cpu_id() as isize)
    }

    fn acquire(&self) {
        push_off();
        if unsafe { self.holding() } {
            panic!("spinlock {} acquire", self.name);
        }
        while self.lock.compare_exchange(false, true,
            Ordering::Acquire, Ordering::Acquire).is_err() {}
        fence(Ordering::SeqCst);
        unsafe { self.cpuid.set(CpuManager::cpu_id() as isize) };
    }

    fn release(&self) {
        if unsafe { !self.holding() } {
            panic!("spinlock {} release", self.name);
        }
        self.cpuid.set(-1);
        fence(Ordering::SeqCst);
        self.lock.store(false, Ordering::Release);
        pop_off();
    }
    
    /// A hole for fork_ret() to release a proc's excl lock
    pub unsafe fn unlock(&self) {
        self.release();
    }
}

pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
    data: &'a mut T,
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.data
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    /// The dropping of the SpinLockGuard will call spinlock's release_lock(),
    /// through its reference to its original spinlock.
    fn drop(&mut self) {
        self.lock.release();
    }
}

impl<'a, T> SpinLockGuard<'a, T> {
    /// Test if the guard is held in the same CPU
    /// Interrupts must be off
    pub unsafe fn holding(&self) -> bool {
        self.lock.holding()
    }
}

/// Copy from crate spin(https://crates.io/crates/spin)
#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;

    pub fn smoke() {
        let m = SpinLock::new((), "smoke");
        m.lock();
        m.lock();
    }
}
