use crate::console;
use crate::spinlock::SpinLock;
use core::fmt;
use core::panic;
use core::sync::atomic::{AtomicBool, Ordering};

/// Pr struct is slightly different,
/// i.e., it is not wrapped in a SpinLock
/// Because we need another field(locking),
/// to represent if we want to use the spinlock when printing.
/// This trick can make `panic` print something to the console quicker.
struct Pr {
    locking: AtomicBool,
    lock: SpinLock<()>,
}

impl Pr {
    fn print(&self, c: u8) {
        console::consputc(c);
    }
}

impl fmt::Write for Pr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.print(byte);
        }
        Ok(())
    }
}

static mut PR: Pr = Pr {
    locking: AtomicBool::new(true),
    lock: SpinLock::new((), "pr"),
};

/// used only in printf's print macro
///
/// note: it needs to be pub because it is used in macro_rules,
///     which access this fn from the crate-level
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;

    unsafe {
        if PR.locking.load(Ordering::Relaxed) {
            let guard = PR.lock.lock();
            PR.write_fmt(args).expect("_print: error");
            drop(guard);
        } else {
            PR.write_fmt(args).expect("_print: error");
        }
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
fn panic(info: &panic::PanicInfo) -> ! {
    crate::println!("{}", info);
    loop {}
}

#[no_mangle]
fn abort() -> ! {
    panic!("abort");
}

#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;
    use crate::consts::param;
    use crate::proc::cpu_id;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn println_mul_hart() {
        let cpu_id = unsafe { cpu_id() };

        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != param::NSMP as u8 {}

        for i in 0..10 {
            println!("println_mul_hart{}: hart {}", i, cpu_id);
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
