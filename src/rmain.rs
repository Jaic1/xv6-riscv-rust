use core::sync::atomic::{AtomicBool, Ordering};

use crate::driver::{virtio_disk::DISK, console};
use crate::register::tp;
use crate::fs::BCACHE;
use crate::mm::kalloc::KERNEL_HEAP;
use crate::mm::{kvm_init, kvm_init_hart};
use crate::plic;
use crate::process::{PROC_MANAGER, CPU_MANAGER};
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
    // explicitly use tp::read here
    let cpuid = tp::read();
    
    if cpuid == 0 {
        console::init();
        println!();
        println!("xv6-riscv-rust is booting");
        println!();
        KERNEL_HEAP.kinit();
        kvm_init(); // init kernel page table
        PROC_MANAGER.proc_init(); // process table
        kvm_init_hart(); // trun on paging
        trap_init_hart(); // install kernel trap vector
        plic::init();
        plic::init_hart(cpuid);
        BCACHE.binit();             // buffer cache
        DISK.lock().init();         // emulated hard disk
        PROC_MANAGER.user_init();   // first user process

        STARTED.store(true, Ordering::SeqCst);
    } else {
        while !STARTED.load(Ordering::SeqCst) {}

        println!("hart {} starting", cpuid);
        kvm_init_hart(); // turn on paging
        trap_init_hart(); // install kernel trap vector
        plic::init_hart(cpuid); // ask PLIC for device interrupts
    }

    #[cfg(feature = "unit_test")]
    super::test_main_entry();

    CPU_MANAGER.scheduler();
}
