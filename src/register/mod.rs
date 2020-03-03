//! register modules defined in this file are only used once in start.rs

pub mod clint;
pub mod mie;
pub mod mstatus;
pub mod satp;
pub mod sie;
pub mod sstatus;

/// medeleg
pub mod medeleg {
    pub unsafe fn write(medeleg: usize) {
        asm!("csrw medeleg, $0"::"r"(medeleg)::"volatile");
    }
}

/// mepc
pub mod mepc {
    pub unsafe fn write(mepc: usize) {
        asm!("csrw mepc, $0"::"r"(mepc)::"volatile");
    }
}

/// mhartid
pub mod mhartid {
    pub unsafe fn read() -> usize {
        let ret: usize;
        asm!("csrr $0, mhartid":"=r"(ret):::"volatile");
        ret
    }
}

/// mideleg
pub mod mideleg {
    pub unsafe fn write(mideleg: usize) {
        asm!("csrw mideleg, $0"::"r"(mideleg)::"volatile");
    }
}

/// mscratch
pub mod mscratch {
    pub unsafe fn write(mscratch: usize) {
        asm!("csrw mscratch, $0"::"r"(mscratch)::"volatile");
    }
}

/// mtvec
pub mod mtvec {
    pub unsafe fn write(mtvec: usize) {
        asm!("csrw mtvec, $0"::"r"(mtvec)::"volatile");
    }
}

/// tp
pub mod tp {
    pub unsafe fn read() -> usize {
        let ret: usize;
        asm!("mv $0, tp":"=r"(ret):::"volatile");
        ret
    }

    pub unsafe fn write(tp: usize) {
        asm!("mv tp, $0"::"r"(tp)::"volatile");
    }
}
