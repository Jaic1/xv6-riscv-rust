use core::{sync::atomic::Ordering, num::Wrapping, ptr};
use core::convert::Into;

use crate::{consts::{UART0, driver::UART_BUF}, spinlock::SpinLock};
use crate::process::{CPU_MANAGER, PROC_MANAGER};
use crate::process::{push_off, pop_off};

use super::PANICKED;
use super::console;

macro_rules! Reg {
    ($reg: expr) => {
        Into::<usize>::into(UART0) + $reg
    };
}

macro_rules! ReadReg {
    ($reg: expr) => {
        unsafe { ptr::read_volatile(Reg!($reg) as *const u8) }
    };
}

macro_rules! WriteReg {
    ($reg: expr, $value: expr) => {
        unsafe {
            ptr::write_volatile(Reg!($reg) as *mut u8, $value);
        }
    };
}

/// Init the Uart device.
pub(super) fn init() {
    // disable interrupts.
    WriteReg!(IER, 0x00);

    // special mode to set baud rate.
    WriteReg!(LCR, 0x80);

    // LSB for baud rate of 38.4K.
    WriteReg!(0, 0x03);

    // MSB for baud rate of 38.4K.
    WriteReg!(1, 0x00);

    // leave set-baud mode,
    // and set word length to 8 bits, no parity.
    WriteReg!(LCR, 0x03);

    // reset and enable FIFOs.
    WriteReg!(FCR, 0x07);

    // enable receive interrupts.
    WriteReg!(IER, 0x03);
}

/// Non-blocking write to uart device.
pub(super) fn putc_sync(c: u8) {
    push_off();
    if PANICKED.load(Ordering::Relaxed) {
        loop {}
    }
    while !is_idle() {}
    WriteReg!(THR, c);
    pop_off();
}

pub static UART: SpinLock<Uart> = SpinLock::new(
    Uart {
        buf: [0; UART_BUF],
        ri: Wrapping(0),
        wi: Wrapping(0),
    },
    "uart",
);

impl SpinLock<Uart> {
    /// Put a u8 to the uart buffer(in the kernel).
    /// It might sleep if the buffer is full.
    pub fn putc(&self, c: u8) {
        let mut uart = self.lock();

        if PANICKED.load(Ordering::Relaxed) {
            loop {}
        }

        loop {
            if uart.wi == uart.ri + Wrapping(UART_BUF) {
                let p = unsafe { CPU_MANAGER.my_proc() };
                p.sleep(&uart.ri as *const Wrapping<_> as usize, uart);
                uart = self.lock();
            } else {
                let wi = uart.wi.0 % UART_BUF;
                uart.buf[wi] = c;
                uart.wi += Wrapping(1);
                uart.transmit();
                break
            }
        }
    }

    /// Uart's interrupt handler.
    /// It receives input data and transmit buffered data.
    pub fn intr(&self) {
        // receive
        loop {
            let c: u8;
            if ReadReg!(LSR) & 1 > 0 {
                c = ReadReg!(RHR);
            } else {
                break
            }
            console::intr(c);
        }

        // transmit
        self.lock().transmit();
    }
}

pub struct Uart {
    buf: [u8; UART_BUF],
    ri: Wrapping<usize>,
    wi: Wrapping<usize>,
}

impl Uart {
    /// Transmit the buffer content if UART device is idle.
    fn transmit(&mut self) {
        while self.wi != self.ri && is_idle() {
            let ri = self.ri.0 % UART_BUF;
            let c = self.buf[ri];
            self.ri += Wrapping(1);
            unsafe { PROC_MANAGER.wakeup(&self.ri as *const Wrapping<_> as usize); }
            WriteReg!(THR, c);
        }
    }
}

// 16550 UART chip's control registers
// usefule reference: http://byterunner.com/16550.html
const RHR: usize = 0;       // receive holding register
const THR: usize = 0;       // transmit holding register
const IER: usize = 1;       // interrupt enable register
const FCR: usize = 2;       // FIFO control register
const ISR: usize = 2;       // interrupt status register
const LCR: usize = 3;       // line control register
const LSR: usize = 5;       // line status register

/// Read the LSR to see if it is able to transmit data.
#[inline]
fn is_idle() -> bool {
    ReadReg!(LSR) & (1 << 5) > 0
}
