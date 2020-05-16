//! self-implementation of Box smart pointer
//! some details might be different

use core::ops::{Deref, DerefMut};
use core::ptr::Unique;

use crate::mm::{kalloc, kfree};

pub trait PageAligned {}

pub struct Box<T>(Unique<T>);

impl<T: PageAligned> Box<T> {
    pub fn new() -> Option<Box<T>> {
        match unsafe { kalloc() } {
            Some(ptr) => Some(Self(Unique::new(ptr as *mut T).unwrap())),
            None => None,
        }
    }

    pub fn into_raw(self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T> Deref for Box<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}

impl<T> Drop for Box<T> {
    fn drop(&mut self) {
        unsafe { kfree(self.0.as_ptr() as *mut u8) }
    }
}
