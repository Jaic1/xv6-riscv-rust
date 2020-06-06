//! Supervisor Interrupt Pending

const SSIP: usize = 1 << 1;

#[inline]
unsafe fn read() -> usize {
    let ret: usize;
    llvm_asm!("csrr $0, sie":"=r"(ret):::"volatile");
    ret
}

#[inline]
unsafe fn write(x: usize) {
    llvm_asm!("csrw sie, $0"::"r"(x)::"volatile");
}

pub fn clear_ssip() {
    unsafe {
        write(read() & !SSIP);
    }
}
