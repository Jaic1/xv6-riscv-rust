//! main initialization process after hardware initialization

use core::sync::atomic::{AtomicBool, Ordering};

use crate::driver::virtio::disk_init;
use crate::fs;
use crate::mm::{kinit, kvm_init, kvm_init_hart};
use crate::plic;
use crate::process::{cpu_id, my_cpu, PROC_MANAGER};
use crate::trap::trap_init_hart;

/// Used by hart 0 to communicate with other harts.
/// When hart 0 finished some initial work,
/// it sets started to true to tell other harts that they can run
///
/// note: actually a simple Bool would be enough,
///     because it is only written once, but just...
static STARTED: AtomicBool = AtomicBool::new(false);

/// start() jumps here in supervisor mode on all CPUs.
pub unsafe fn rust_main() -> ! {
    if cpu_id() == 0 {
        crate::console::consoleinit();
        println!();
        println!("xv6-riscv-rust is booting");
        println!();
        kinit();
        kvm_init();                 // init kernel page table
        PROC_MANAGER.proc_init();   // process table
        kvm_init_hart();            // trun on paging
        trap_init_hart();           // install kernel trap vector
        plic::init();
        plic::init_hart();
        fs::binit();                // buffer cache
        disk_init();                // emulated hard disk
        PROC_MANAGER.user_init();   // first user process

        STARTED.store(true, Ordering::SeqCst);
    } else {
        while !STARTED.load(Ordering::SeqCst) {}

        println!("hart {} starting", cpu_id());
        kvm_init_hart();            // turn on paging
        trap_init_hart();           // install kernel trap vector
        plic::init_hart();          // ask PLIC for device interrupts

        #[cfg(not(feature = "unit_test"))]
        loop {}
    }

    #[cfg(feature = "unit_test")]
    super::test_main_entry();

    // each cpu's lifetime start here
    let c = my_cpu();
    c.scheduler();
}
