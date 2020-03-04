use crate::console;
use core::fmt;
use core::panic;

#[panic_handler]
fn panic(info: &panic::PanicInfo) -> ! {
    crate::println!("{}", info);
    loop {}
}

#[no_mangle]
fn abort() -> ! {
    panic!("abort");
}

struct Print {
    locking: bool,
}

impl Print {
    fn print(&self, c: u8) {
        console::consputc(c);
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

static mut PRINT: Print = Print { locking: false };

// used only in printf's print macro
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    unsafe {
        PRINT.write_fmt(args).expect("_print: error");
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
