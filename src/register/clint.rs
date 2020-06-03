//! CLINT operation, refer doc/FU540-C000-v1.0.pdf for detail.
//!
//! note: mtime and mtimecmp are both 64-bit registers
//!     they will not probably exceed the time the machine can run.

use core::ptr;
use core::convert::Into;

use crate::consts::{CLINT_MTIME, CLINT_MTIMECMP};

#[inline]
unsafe fn read_mtime() -> u64 {
    ptr::read_volatile(Into::<usize>::into(CLINT_MTIME) as *const u64)
}

#[inline]
unsafe fn write_mtimecmp(mhartid: usize, value: u64) {
    let offset = Into::<usize>::into(CLINT_MTIMECMP) + 8 * mhartid;
    ptr::write_volatile(offset as *mut u64, value);
}

pub unsafe fn add_mtimecmp(mhartid: usize, interval: u64) {
    let value = read_mtime();
    write_mtimecmp(mhartid, value + interval);
}

pub unsafe fn read_mtimecmp(mhartid: usize) -> u64 {
    let offset = Into::<usize>::into(CLINT_MTIMECMP) + 8 * mhartid;
    ptr::read_volatile(offset as *const u64)
}
