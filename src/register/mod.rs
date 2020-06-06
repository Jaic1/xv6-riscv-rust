//! register modules defined in this file are only used once in start.rs

pub mod clint;
pub mod mie;
pub mod mstatus;
pub mod satp;
pub mod sie;
pub mod sip;
pub mod sstatus;
pub mod scause;

/// medeleg
pub mod medeleg {
    pub unsafe fn write(medeleg: usize) {
        llvm_asm!("csrw medeleg, $0"::"r"(medeleg)::"volatile");
    }
}

/// mepc
pub mod mepc {
    pub unsafe fn write(mepc: usize) {
        llvm_asm!("csrw mepc, $0"::"r"(mepc)::"volatile");
    }
}

/// mhartid
pub mod mhartid {
    pub unsafe fn read() -> usize {
        let ret: usize;
        llvm_asm!("csrr $0, mhartid":"=r"(ret):::"volatile");
        ret
    }
}

/// mideleg
pub mod mideleg {
    pub unsafe fn write(mideleg: usize) {
        llvm_asm!("csrw mideleg, $0"::"r"(mideleg)::"volatile");
    }
}

/// mscratch
pub mod mscratch {
    pub unsafe fn write(mscratch: usize) {
        llvm_asm!("csrw mscratch, $0"::"r"(mscratch)::"volatile");
    }
}

/// mtvec
pub mod mtvec {
    pub unsafe fn write(mtvec: usize) {
        llvm_asm!("csrw mtvec, $0"::"r"(mtvec)::"volatile");
    }
}

/// tp
pub mod tp {
    pub unsafe fn read() -> usize {
        let ret: usize;
        llvm_asm!("mv $0, tp":"=r"(ret):::"volatile");
        ret
    }

    pub unsafe fn write(tp: usize) {
        llvm_asm!("mv tp, $0"::"r"(tp)::"volatile");
    }
}

/// stvec
pub mod stvec {
    pub unsafe fn write(stvec: usize) {
        llvm_asm!("csrw stvec, $0"::"r"(stvec)::"volatile");
    }
}

/// sepc
/// machine exception program counter, holds the
/// instruction address to which a return from
/// exception will go.(from xv6-riscv)
pub mod sepc {
    pub fn read() -> usize {
        let ret: usize;
        unsafe {llvm_asm!("csrr $0, sepc":"=r"(ret):::"volatile");}
        ret
    }

    pub fn write(sepc: usize) {
        unsafe {llvm_asm!("csrw sepc, $0"::"r"(sepc)::"volatile");}
    }
}

/// stval
/// contains supervisor trap value
pub mod stval {
    pub fn read() -> usize {
        let ret: usize;
        unsafe {llvm_asm!("csrr $0, stval":"=r"(ret):::"volatile");}
        ret
    }
}
