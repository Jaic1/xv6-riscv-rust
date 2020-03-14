pub mod addr;
pub mod kalloc;

const PGSIZE: usize = 4096;
const PGSHIFT: u8 = 12;

#[inline]
pub fn pg_round_up(addr: usize) -> usize {
    (addr + PGSIZE - 1) & !(PGSIZE - 1)
}

#[inline]
pub fn pg_round_down(addr: usize) -> usize {
    addr & !(PGSIZE - 1)
}
