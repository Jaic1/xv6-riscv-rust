use crate::consts::{CLINT_MTIMECMP, NCPU};
use crate::register::{
    clint, medeleg, mepc, mhartid, mideleg, mie, mscratch, mstatus, mtvec, satp, tp,
};
use crate::rmain::rust_main;

/// for each cpu, only 6 of 32 usize are used, others are reserved.
static mut MSCRATCH0: [usize; NCPU * 32] = [0; NCPU * 32];

#[no_mangle]
pub unsafe fn start() -> ! {
    // set M Previous Privilege mode to Supervisor, for mret.
    mstatus::set_mpp(mstatus::MPP::Supervisor);

    // set M Exception Program Counter to main, for mret.
    mepc::write(rust_main as usize);

    // disable paging for now.
    satp::write(0);

    // delegate all interrupts and exceptions to supervisor mode.
    medeleg::write(0xffff);
    mideleg::write(0xffff);

    // ask for clock interrupts.
    timerinit();

    // keep each CPU's hartid in its tp register, for cpuid().
    let id = mhartid::read();
    tp::write(id);

    // switch to supervisor mode and jump to main().
    asm!("mret"::::"volatile");

    // cannot panic or print here
    loop {}
}

/// set up to receive timer interrupts in machine mode,
/// which arrive at timervec in kernelvec.S,
/// which turns them into software interrupts for
/// devintr() in trap.rs.
unsafe fn timerinit() {
    // each CPU has a separate source of timer interrupts.
    let id = mhartid::read();

    // ask the CLINT for a timer interrupt.
    let interval: u64 = 1000000; // cycles; about 1/10th second in qemu.
    clint::add_mtimecmp(id, interval);

    // prepare information in scratch[] for timervec.
    // scratch[0..3] : space for timervec to save registers.
    // scratch[4] : address of CLINT MTIMECMP register.
    // scratch[5] : desired interval (in cycles) between timer interrupts.
    let offset = 32 * id;
    MSCRATCH0[offset + 4] = CLINT_MTIMECMP + 8 * id;
    MSCRATCH0[offset + 5] = interval as usize;
    mscratch::write((MSCRATCH0.as_ptr() as usize) + offset * core::mem::size_of::<usize>());

    // set the machine-mode trap handler.
    extern "C" {
        fn timervec();
    }
    mtvec::write(timervec as usize);

    // enable machine-mode interrupts.
    mstatus::set_mie();

    // enable machine-mode timer interrupts.
    mie::set_mtie();
}
