//! Trap handler between user/kernel space and kernel space
//! Mostly adopted from xv6-riscv

use crate::consts::{TRAMPOLINE, TRAPFRAME};
use crate::register::{stvec, sstatus, sepc, stval, sip, scause::{self, ScauseType}};
use crate::process::{cpu_id, my_cpu};
use crate::spinlock::SpinLock;

pub unsafe fn trap_init_hart() {
    extern "C" {
        fn kernelvec();
    }

    stvec::write(kernelvec as usize);
}

/// uservec in trampoline.S jumps here 
#[no_mangle]
pub unsafe extern fn user_trap() {
    if !sstatus::is_from_user() {
        panic!("user_trap: not from user mode, sstatus={:#x}", sstatus::read());
    }

    // switch the trap handler to kerneltrap()
    extern "C" {fn kernelvec();}
    stvec::write(kernelvec as usize);

    handle_trap(true);

    user_trap_ret();
}

/// Return to user space
pub unsafe fn user_trap_ret() -> ! {
    // disable interrupts and prepare sret to user mode
    sstatus::intr_off();
    sstatus::user_ret_prepare();

    // send interrupts and exceptions to uservec/trampoline in trampoline.S
    stvec::write(TRAMPOLINE.into());

    // let the current cpu and process prepare for the sret
    let c = my_cpu();
    let satp = c.user_ret_prepare();

    // call userret with virtual address
    extern "C" {
        fn trampoline();
        fn userret();
    }
    let distance = userret as usize - trampoline as usize;
    let userret_virt: extern "C" fn(usize, usize) -> ! =
        core::mem::transmute(Into::<usize>::into(TRAMPOLINE) + distance);
    userret_virt(TRAPFRAME.into(), satp);
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

    handle_trap(false);

    // the yield() may have caused some traps to occur,
    // so restore trap registers for use by kernelvec.S's sepc instruction.
    sepc::write(local_sepc);
    sstatus::write(local_sstatus);
}

/// Check the type of trap, i.e., interrupt or exception
/// under the supervisor mode
/// it is from xv6-riscv's devintr()
fn handle_trap(is_user: bool) {
    match scause::get_scause() {
        ScauseType::IntSExt => {
            if is_user {
                // TODO
            } else {
                panic!("handle_trap: expect no software external interrupt");
            }
        }
        ScauseType::IntSSoft => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.

            let cid = unsafe {cpu_id()};

            if cid == 0 {
                clock_intr();
            }

            // acknowledge the software interrupt
            sip::clear_ssip();

            // give up the cpu
            let c = unsafe {my_cpu()};
            if is_user {
                c.try_abondon(-1);
            }
            c.yielding();
        }
        ScauseType::ExcUEcall => {
            if !is_user {
                panic!("handler_trap: ecall from supervisor mode");
            }

            let c = unsafe {my_cpu()};
            c.syscall();
        }
        ScauseType::Unknown => {
            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            if is_user {
                let c = unsafe {my_cpu()};
                c.abondon(-1);
            } else {
                panic!("handle_trap: unknown trap type");
            }
        }
    }
}

static TICKS: SpinLock<usize> = SpinLock::new(0usize, "time");

fn clock_intr() {
    let mut _ticks = TICKS.lock();
    *_ticks += 1;
    drop(_ticks);
}
