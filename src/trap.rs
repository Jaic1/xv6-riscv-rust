use crate::register::{stvec, sstatus, sepc, stval, sip, scause::{self, ScauseType}};
use crate::process::{cpu_id, my_cpu};
use crate::spinlock::SpinLock;

pub unsafe fn trap_init_hart() {
    extern "C" {
        fn kernelvec();
    }

    stvec::write(kernelvec as usize);
}

/// Used to handle kernel space's trap
/// Being called from kernelvec
#[no_mangle]
pub fn kerneltrap() {
    let local_sepc = sepc::read();
    let local_sstatus = sstatus::read();

    if !sstatus::is_from_supervisor() {
        panic!("kerneltrap: not from supervisor mode");
    }
    if sstatus::intr_get() {
        panic!("kerneltrap: interrupts enabled");
    }

    handle_trap();

    // the yield() may have caused some traps to occur,
    // so restore trap registers for use by kernelvec.S's sepc instruction.
    sepc::write(local_sepc);
    sstatus::write(local_sstatus);
}

/// Check the type of trap, i.e., interrupt or exception
/// under the supervisor mode
/// it is from xv6-riscv's devintr()
fn handle_trap() {
    match scause::get_scause() {
        ScauseType::IntSExt => {
            panic!("handle_trap: expect no software external interrupt");
        }
        ScauseType::IntSSoft => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.

            // debug
            println!("handle_trap: handling timer interrupt");

            if unsafe {cpu_id()} == 0 {
                clock_intr();
            }

            sip::clear_ssip();

            // give up the cpu
            let c = unsafe {my_cpu()};
            c.yielding();
        }
        ScauseType::Unknown => {
            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            panic!("handle_trap: unknown trap type");
        }
    }
}

static TICKS: SpinLock<usize> = SpinLock::new(0usize, "time");

fn clock_intr() {
    let mut _ticks = TICKS.lock();
    *_ticks += 1;
    drop(_ticks);
}
