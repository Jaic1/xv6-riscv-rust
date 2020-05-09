use crate::consts::PGSIZE;

pub use addr::{PhysAddr, VirtAddr};
pub use boxed::{Box, PageAligned};
pub use kalloc::{kinit, kalloc, kfree};

mod addr;
mod boxed;
mod kalloc;
mod kvm;
mod pagetable;

#[inline]
fn pg_round_up(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !(PGSIZE - 1)
}

#[inline]
fn pg_round_down(addr: usize) -> usize {
    addr & !(PGSIZE - 1)
}
