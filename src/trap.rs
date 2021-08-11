//! Trap handler between user/kernel space and kernel space

use core::num::Wrapping;
use core::sync::atomic::Ordering;

use crate::{consts::{TRAMPOLINE, TRAPFRAME, UART0_IRQ, VIRTIO0_IRQ}, process::{PROC_MANAGER, Proc}};
use crate::register::{stvec, sstatus, sepc, stval, sip,
    scause::{self, ScauseType}};
use crate::process::{CPU_MANAGER, CpuManager};
use crate::spinlock::SpinLock;
use crate::plic;
use crate::driver::virtio_disk::DISK;
use crate::driver::uart::UART;

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
        panic!("not from user mode, sstatus={:#x}", sstatus::read());
    }

    // switch the trap handler to kerneltrap()
    extern "C" {fn kernelvec();}
    stvec::write(kernelvec as usize);

    let p = CPU_MANAGER.my_proc();

    match scause::get_scause() {
        ScauseType::IntSExt => {
            // this is a supervisor external interrupt, via PLIC.

            let irq = plic::claim();
            if irq as usize == UART0_IRQ {
                UART.intr();
            } else if irq as usize == VIRTIO0_IRQ {
                DISK.lock().intr();
            } else {
                // panic!("unexpected interrupt, irq={}", irq);
            }
            if irq > 0 {
                plic::complete(irq);
            }

            p.check_abondon(-1);
        }
        ScauseType::IntSSoft => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.

            // only cpu 0 inc ticks
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            // acknowledge the software interrupt
            sip::clear_ssip();

            // give up the cpu
            p.check_abondon(-1);
            p.yielding();
        }
        ScauseType::ExcUEcall => {
            p.check_abondon(-1);
            p.syscall();
            p.check_abondon(-1);
        }
        ScauseType::Unknown => {
            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            p.abondon(-1);
        }
    }

    user_trap_ret();
}

/// Return to user space
pub unsafe fn user_trap_ret() -> ! {
    // disable interrupts and prepare sret to user mode
    sstatus::intr_off();
    sstatus::user_ret_prepare();

    // send interrupts and exceptions to uservec/trampoline in trampoline.S
    stvec::write(TRAMPOLINE.into());

    // let the current process prepare for the sret
    let satp = {
        let pd = CPU_MANAGER.my_proc().data.get_mut();
        pd.user_ret_prepare()
    };

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
pub unsafe fn kerneltrap() {
    let local_sepc = sepc::read();
    let local_sstatus = sstatus::read();

    if !sstatus::is_from_supervisor() {
        panic!("not from supervisor mode");
    }
    if sstatus::intr_get() {
        panic!("interrupts enabled");
    }

    match scause::get_scause() {
        ScauseType::IntSExt => {
            // this is a supervisor external interrupt, via PLIC.

            let irq = plic::claim();
            if irq as usize == UART0_IRQ {
                UART.intr();
            } else if irq as usize == VIRTIO0_IRQ {
                DISK.lock().intr();
            } else {
                // panic!("unexpected interrupt, irq={}", irq);
            }
            if irq > 0 {
                plic::complete(irq);
            }
        }
        ScauseType::IntSSoft => {
            // software interrupt from a machine-mode timer interrupt,
            // forwarded by timervec in kernelvec.S.

            // only cpu 0 inc ticks
            if CpuManager::cpu_id() == 0 {
                clock_intr();
            }

            // acknowledge the software interrupt
            sip::clear_ssip();

            // give up the cpu
            CPU_MANAGER.my_cpu_mut().try_yield_proc();
        }
        ScauseType::ExcUEcall => {
            panic!("ecall from supervisor mode");
        }
        ScauseType::Unknown => {
            println!("scause {:#x}", scause::read());
            println!("sepc={:#x} stval={:#x}", sepc::read(), stval::read());
            panic!("unknown trap type");
        }
    }

    // The yielding() may have caused some traps to occur,
    // so restore trap registers for use by kernelvec.S's sepc instruction.
    sepc::write(local_sepc);
    sstatus::write(local_sstatus);
}

static TICKS: SpinLock<Wrapping<usize>> = SpinLock::new(Wrapping(0), "time");

fn clock_intr() {
    let mut guard = TICKS.lock();
    *guard += Wrapping(1);
    unsafe { PROC_MANAGER.wakeup(&TICKS as *const _ as usize); }
    drop(guard);
}

/// Sleep for a specified number of ticks.
pub fn clock_sleep(p: &Proc, count: usize) -> Result<(), ()> {
    let mut guard = TICKS.lock();
    let old_ticks = *guard;
    while (*guard - old_ticks) < Wrapping(count) {
        if p.killed.load(Ordering::Relaxed) {
            return Err(())
        }
        p.sleep(&TICKS as *const _ as usize, guard);
        guard = TICKS.lock();
    }
    Ok(())
}

/// Read the current ticks.
pub fn clock_read() -> usize {
    TICKS.lock().0
}
