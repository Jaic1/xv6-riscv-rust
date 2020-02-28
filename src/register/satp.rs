//! satp register

pub unsafe fn write(satp: usize) {
    asm!("csrw satp, $0"::"r"(satp)::"volatile");
}
