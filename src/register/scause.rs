//! Supervisor Trap Cause

const INTERRUPT: usize = 0x8000000000000000;
const INTERRUPT_SUPERVISOR_SOFTWARE: usize = INTERRUPT + 1;
const INTERRUPT_SUPERVISOR_EXTERNAL: usize = INTERRUPT + 9;

pub enum ScauseType {
    Unknown,
    IntSSoft,
    IntSExt,
}

#[inline]
pub fn read() -> usize {
    let ret: usize;
    unsafe {llvm_asm!("csrr $0, scause":"=r"(ret):::"volatile");}
    ret
}

pub fn get_scause() -> ScauseType {
    let scause = read();
    match scause {
        INTERRUPT_SUPERVISOR_SOFTWARE => ScauseType::IntSSoft,
        INTERRUPT_SUPERVISOR_EXTERNAL => ScauseType::IntSExt,
        _ => ScauseType::Unknown,
    }
}
