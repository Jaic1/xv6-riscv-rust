use core::sync::atomic::{AtomicBool, Ordering};

use crate::process::{PROC_MANAGER, cpu_id, my_cpu};
use crate::mm::{kinit, kvm_init, kvm_init_hart};

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
        kvm_init(); // init kernel page table
        PROC_MANAGER.proc_init(); // process table
        kvm_init_hart(); // trun on paging

        // TODO - user_init();

        STARTED.store(true, Ordering::SeqCst);
    } else {
        while !STARTED.load(Ordering::SeqCst) {}

        println!("hart {} starting", cpu_id());
        kvm_init_hart(); // turn on paging

        // TODO - init other things
        loop {}
    }

    #[cfg(feature = "unit_test")]
        super::test_main_entry();

    // each cpu's lifetime start here?
    let c = my_cpu();
    c.scheduler();
}
