//! sie register

const SSIE: usize = 1 << 1;     // software
const STIE: usize = 1 << 5;     // timer
const SEIE: usize = 1 << 9;     // external

#[inline]
unsafe fn read() -> usize {
    let ret: usize;
    asm!("csrr $0, sie":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn write(x: usize) {
    asm!("csrw sie, $0"::"r"(x)::"volatile");
}

/// enable all software interrupts
/// still need to set SIE bit in sstatus
pub fn intr_on() {
    unsafe {
        let mut sie = read();
        sie |= SSIE | STIE | SEIE;
        write(sie);
    }
}