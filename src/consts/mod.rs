use core::ops::{Add, Sub};
use core::convert::From;

pub use memlayout::*;
pub use param::*;
pub use riscv::*;

pub mod fs;
pub mod driver;

mod memlayout;
mod param;
mod riscv;

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ConstAddr(usize);

impl ConstAddr {
    /// due to E0015's const restriction
    pub const fn const_add(&self, adder: usize) -> Self {
        Self(self.0 + adder)
    }

    /// due to E0015's const restriction
    pub const fn const_sub(&self, suber: usize) -> Self {
        Self(self.0 - suber)
    }
}

impl Add for ConstAddr {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for ConstAddr {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl From<ConstAddr> for usize {
    fn from(const_addr: ConstAddr) -> Self {
        const_addr.0
    }
}
