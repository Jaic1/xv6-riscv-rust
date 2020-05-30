use core::sync::atomic::{AtomicBool, Ordering};

use crate::proc::{cpu_id, my_cpu};
use crate::mm::{kinit, kvm_init, kvm_init_hart};
use crate::proc::proc_init;

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
        proc_init(); // process table
        kvm_init_hart(); // trun on paging

        // TODO - init other things

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

    let c = my_cpu();
    c.scheduler();
}
