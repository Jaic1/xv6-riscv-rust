//! mstatus register

use bit_field::BitField;

#[inline]
unsafe fn r_mstatus() -> usize {
    let ret: usize;
    asm!("csrr $0, mstatus":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn w_mstatus(x: usize) {
    asm!("csrw mstatus, $0"::"r"(x)::"volatile");
}

/// Machine Previous Privilege Mode
pub enum MPP {
    User = 0,
    Supervisor = 1,
    Machine = 3,
}

/// Set MPP field in mstatus
pub unsafe fn set_mpp(mpp: MPP) {
    let mut mstatus = r_mstatus();
    mstatus.set_bits(11..13, mpp as usize);
    w_mstatus(mstatus);
}
