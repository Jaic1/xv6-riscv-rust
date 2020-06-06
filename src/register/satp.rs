//! satp register

#[inline]
pub fn read() -> usize {
    let ret;
    unsafe {
        llvm_asm!("csrr $0, satp":"=r"(ret):::"volatile");
    }
    ret
}

#[inline]
pub fn write(satp: usize) {
    unsafe {
        llvm_asm!("csrw satp, $0"::"r"(satp)::"volatile");
    }
}
