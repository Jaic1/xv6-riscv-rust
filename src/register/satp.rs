//! satp register

pub unsafe fn set(satp: usize) {
    asm!("csrw satp, $0"::"r"(satp)::"volatile");
}
