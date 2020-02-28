//! mstatus register

use bit_field::BitField;

#[inline]
unsafe fn read() -> usize {
    let ret: usize;
    asm!("csrr $0, mstatus":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn write(x: usize) {
    asm!("csrw mstatus, $0"::"r"(x)::"volatile");
}

/// Machine Previous Privilege Mode
pub enum MPP {
    User = 0,
    Supervisor = 1,
    Machine = 3,
}

/// set MPP field
pub unsafe fn set_mpp(mpp: MPP) {
    let mut mstatus = read();
    mstatus.set_bits(11..13, mpp as usize);
    write(mstatus);
}

/// set MIE field
pub unsafe fn set_mie() {
    let mut mstatus = read();
    mstatus.set_bit(3, true);
    write(mstatus);
}
