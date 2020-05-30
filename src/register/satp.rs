//! satp register

pub unsafe fn write(satp: usize) {
    llvm_asm!("csrw satp, $0"::"r"(satp)::"volatile");
}
