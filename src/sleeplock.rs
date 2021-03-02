//! sleeplock

use core::ops::{Deref, DerefMut, Drop};
use core::cell::{Cell, UnsafeCell};

use crate::process::{CPU_MANAGER, PROC_MANAGER};
use crate::spinlock::SpinLock;

pub struct SleepLock<T: ?Sized> {
    lock: SpinLock<()>,
    locked: Cell<bool>,
    name: &'static str,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SleepLock<T> {}
// not needed
// unsafe impl<T: ?Sized + Send> Send for SleepLock<T> {}

impl<T> SleepLock<T> {
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            lock: SpinLock::new((), "sleeplock"),
            locked: Cell::new(false),
            name,
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SleepLock<T> {
    /// blocking, might sleep if this sleeplock is already locked
    pub fn lock(&self) -> SleepLockGuard<'_, T> {
        let mut guard = self.lock.lock();
        while self.locked.get() {
            unsafe {
                CPU_MANAGER.my_proc().sleep(self.locked.as_ptr() as usize, guard);
            }
            guard = self.lock.lock();
        }
        self.locked.set(true);
        drop(guard);
        SleepLockGuard {
            lock: &self,
            data: unsafe { &mut *self.data.get() }
        }
    }

    /// Called by its guard when dropped
    fn unlock(&self) {
        let guard = self.lock.lock();
        self.locked.set(false);
        self.wakeup();
        drop(guard);
    }

    fn wakeup(&self) {
        unsafe {
            PROC_MANAGER.wakeup(self.locked.as_ptr() as usize);
        }
    }
}

pub struct SleepLockGuard<'a, T: ?Sized> {
    lock: &'a SleepLock<T>,
    data: &'a mut T,
}

impl<'a, T: ?Sized> Deref for SleepLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SleepLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.data
    }
}

impl<'a, T: ?Sized> Drop for SleepLockGuard<'a, T> {
    /// The dropping of the SpinLockGuard will call spinlock's release_lock(),
    /// through its reference to its original spinlock.
    fn drop(&mut self) {
        self.lock.unlock();
    }
}
