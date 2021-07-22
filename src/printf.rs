use core::fmt;
use core::panic;
use core::sync::atomic::Ordering;

use crate::driver::{console, PANICKED};
use crate::spinlock::SpinLock;

/// ZST Print struct to sequence printing printing across multiple CPUS.
struct Print;

impl Print {
    fn print(&self, c: u8) {
        console::putc(c);
    }
}

impl fmt::Write for Print {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.print(byte);
        }
        Ok(())
    }
}

/// Used only in printf's print macro.
///
/// Note: It needs to be pub because it is used in macro_rules,
///     which access this fn from the crate-level.
pub fn _print(args: fmt::Arguments<'_>) {
    use fmt::Write;
    static PRINT: SpinLock<()> = SpinLock::new((), "print");

    if PANICKED.load(Ordering::Relaxed) {
        // no need to lock
        Print.write_fmt(args).expect("_print: error");
    } else {
        let guard = PRINT.lock();
        Print.write_fmt(args).expect("_print: error");
        drop(guard);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::printf::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {$crate::print!("\n")};
    ($fmt:expr) => {$crate::print!(concat!($fmt, "\n"))};
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!($fmt, "\n"), $($arg)*)
    };
}

#[panic_handler]
fn panic(info: &panic::PanicInfo<'_>) -> ! {
    crate::println!("{}", info);
    PANICKED.store(true, Ordering::Relaxed);
    loop {}
}

#[no_mangle]
fn abort() -> ! {
    panic!("abort");
}

#[cfg(feature = "unit_test")]
pub mod tests {
    use crate::consts::NSMP;
    use crate::proc::cpu_id;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn println_simo() {
        let cpu_id = unsafe { cpu_id() };

        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != NSMP as u8 {}

        for i in 0..10 {
            println!("println_mul_hart{}: hart {}", i, cpu_id);
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
