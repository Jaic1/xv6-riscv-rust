//! Console driver for user input and output.

use core::num::Wrapping;
use core::sync::atomic::Ordering;

use crate::consts::driver::*;
use crate::spinlock::SpinLock;
use crate::mm::Address;
use crate::process::{CPU_MANAGER, PROC_MANAGER};

use super::uart;

// SAFETY: Called only once in rmain.rs:rust_main.
pub unsafe fn init() {
    uart::init();
}

/// Read from console `tot` bytes to `dst`,
/// which might be a virtual or kernel [`Address`].
pub(super) fn read(mut dst: Address, tot: u32) -> Result<u32, ()> {
    let mut console = CONSOLE.lock();

    let mut left = tot;
    while left > 0 {
        // if no available data in console buf
        // wait until the console device write some data
        while console.ri == console.wi {
            let p = unsafe { CPU_MANAGER.my_proc() };
            if p.killed.load(Ordering::Relaxed) {
                return Err(())
            }
            p.sleep(&console.ri as *const Wrapping<_> as usize, console);
            console = CONSOLE.lock();
        }

        // read
        let c = console.buf[console.ri.0 % CONSOLE_BUF];
        console.ri += Wrapping(1);

        // encounter EOF
        // return earlier
        if c == CTRL_EOT {
            if left < tot {
                console.ri -= Wrapping(1);
            }
            break;
        }

        // copy to user/kernel space memory
        if dst.copy_out(&c as *const u8, 1).is_err() {
            break;
        }

        // update
        dst = dst.offset(1);
        left -= 1;

        // encounter a line feed
        if c == CTRL_LF {
            break;
        }
    }

    Ok(tot - left)
}

/// Write to console `tot` bytes from `src`,
/// which might be a virtual or kernel [`Address`].
pub(super) fn write(mut src: Address, tot: u32) -> Result<u32, ()> {
    for i in 0..tot {
        let mut c = 0u8;
        if src.copy_in(&mut c as *mut u8, 1).is_err() {
            return Ok(i)
        }
        uart::UART.putc(c);
        src = src.offset(1);
    }
    Ok(tot)
}

/// Put a single character to console.
pub(crate) fn putc(c: u8) {
    if c == CTRL_BS {
        uart::putc_sync(CTRL_BS);
        uart::putc_sync(b' ');
        uart::putc_sync(CTRL_BS);
    } else {
        uart::putc_sync(c);
    }
}

/// The console interrupt handler.
/// The normal routine is:
/// 1. user input;
/// 2. uart handle interrupt;
/// 3. console handle interrupt;
/// 4. console echo back input or do extra controlling.
pub(super) fn intr(c: u8) {
    let mut console = CONSOLE.lock();

    match c {
        CTRL_PRINT_PROCESS => {
            todo!("print process list to debug")
        },
        CTRL_BS_LINE => {
            while console.ei != console.wi &&
                console.buf[(console.ei-Wrapping(1)).0 % CONSOLE_BUF] != CTRL_LF
            {
                console.ei -= Wrapping(1);
                putc(CTRL_BS);
            }
        },
        CTRL_BS | CTRL_DEL => {
            if console.ei != console.wi {
                console.ei -= Wrapping(1);
                putc(CTRL_BS);
            }
        }
        _ => {
            // echo back
            if c != 0 && (console.ei - console.ri).0 < CONSOLE_BUF {
                let c = if c == CTRL_CR { CTRL_LF } else { c };
                putc(c);
                let ei = console.ei.0 % CONSOLE_BUF;
                console.buf[ei] = c;
                console.ei += Wrapping(1);
                if c == CTRL_LF || c == CTRL_EOT || (console.ei - console.ri).0 == CONSOLE_BUF {
                    console.wi = console.ei;
                    unsafe { PROC_MANAGER.wakeup(&console.ri as *const Wrapping<_> as usize); }
                }
            }
        },
    }
}

static CONSOLE: SpinLock<Console> = SpinLock::new(
    Console {
        buf: [0; CONSOLE_BUF],
        ri: Wrapping(0),
        wi: Wrapping(0),
        ei: Wrapping(0),
    },
    "console",
);

struct Console {
    buf: [u8; CONSOLE_BUF],
    // read index
    ri: Wrapping<usize>,
    // write index
    wi: Wrapping<usize>,
    // edit index
    ei: Wrapping<usize>,
}
