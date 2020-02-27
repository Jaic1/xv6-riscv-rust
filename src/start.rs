use crate::register::{mepc, mstatus, satp};
use crate::rmain::rust_main;

#[no_mangle]
pub unsafe fn start() -> ! {
    // set M Previous Privilege mode to Supervisor, for mret.
    mstatus::set_mpp(mstatus::MPP::Supervisor);

    // set M Exception Program Counter to main, for mret.
    mepc::set(rust_main as usize);

    // disable paging for now.
    satp::set(0);

    todo!();

    // // delegate all interrupts and exceptions to supervisor mode.
    // w_medeleg(0xffff);
    // w_mideleg(0xffff);

    // // ask for clock interrupts.
    // timerinit();

    // // keep each CPU's hartid in its tp register, for cpuid().
    // int id = r_mhartid();
    // w_tp(id);

    // // switch to supervisor mode and jump to main().
    // asm volatile("mret");
}
