//! mepc register
//! only used once in start.rs

pub unsafe fn set(mepc: usize) {
    asm!("csrw mepc, $0"::"r"(mepc)::"volatile");
}
