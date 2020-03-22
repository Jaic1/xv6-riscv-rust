pub mod kalloc;

mod addr;
mod kvm;
mod pagetable;

const PGSIZE: usize = 4096;
const PGSHIFT: u8 = 12;

#[inline]
fn pg_round_up(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !(PGSIZE - 1)
}

#[inline]
fn pg_round_down(addr: usize) -> usize {
    addr & !(PGSIZE - 1)
}
