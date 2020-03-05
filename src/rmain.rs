use crate::proc::cpu_id;
use core::sync::atomic::{fence, AtomicBool, Ordering};

/// Used by hart 0 to communicate with other harts.
/// When hart 0 finished some initial work,
/// it sets started to true to tell other harts that they can run
///
/// note: actually a simple Bool would be enough,
///     because it is only written once, but just...
static STARTED: AtomicBool = AtomicBool::new(false);

/// start() jumps here in supervisor mode on all CPUs.
pub fn rust_main() -> ! {
    if unsafe { cpu_id() } == 0 {
        crate::console::consoleinit();

        fence(Ordering::SeqCst);
        STARTED.store(true, Ordering::Relaxed);
    } else {
        while !STARTED.load(Ordering::Relaxed) {}

        println!("hart {} starting", unsafe { cpu_id() });
    }

    #[cfg(feature = "unit_test")]
    super::test_main_entry();
    panic!("rust_main: end");
}
