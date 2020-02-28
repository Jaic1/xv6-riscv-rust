//! mie register

use bit_field::BitField;

#[inline]
unsafe fn read() -> usize {
    let ret: usize;
    asm!("csrr $0, mie":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn write(x: usize) {
    asm!("csrw mie, $0"::"r"(x)::"volatile");
}

/// set MTIE field
pub unsafe fn set_mtie() {
    let mut mie = read();
    mie.set_bit(7, true);
    write(mie);
}
