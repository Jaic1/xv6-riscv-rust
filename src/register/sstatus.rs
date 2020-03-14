//! sstatus register

const SIE: usize = 1 << 1; // supervisor interrupt enable

#[inline]
unsafe fn read() -> usize {
    let ret: usize;
    asm!("csrr $0, sstatus":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn write(x: usize) {
    asm!("csrw sstatus, $0"::"r"(x)::"volatile");
}

/// set SIE to enable device interrupts
/// still need to set relevant bit in sie register
pub fn intr_on() {
    unsafe {
        write(read() | SIE);
    }
}

/// disable device interrupts
pub fn intr_off() {
    unsafe {
        write(read() & !SIE);
    }
}

/// are device interrupts enabled?
pub fn intr_get() -> bool {
    unsafe {
        let x = read();
        (x & SIE) != 0
    }
}
