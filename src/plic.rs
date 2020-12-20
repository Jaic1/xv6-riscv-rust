//! the riscv Platform Level Interrupt Controller (PLIC)

use core::ptr;

use crate::process::CpuManager;
use crate::consts::{PLIC, UART0_IRQ, VIRTIO0_IRQ};

pub unsafe fn init() {
    // set desired IRQ priorities non-zero (otherwise disabled)
    write(UART0_IRQ*4, 1);
    write(VIRTIO0_IRQ*4, 1);
}

pub unsafe fn init_hart(hart: usize) {
    write(SENABLE+SENABLE_HART*hart, (1<<UART0_IRQ)|(1<<VIRTIO0_IRQ));
    write(SPRIORITY+SPRIORITY_HART*hart, 0);
}

/// ask the PLIC what interrupt we should serve
pub fn claim() -> u32 {
    let hart: usize = unsafe {CpuManager::cpu_id()};
    read(SCLAIM+SCLAIM_HART*hart)
}

/// tell the PLIC we've served this IRQ
pub fn complete(irq: u32) {
    let hart: usize = unsafe {CpuManager::cpu_id()};
    write(SCLAIM+SCLAIM_HART*hart, irq);
}

// qemu puts programmable interrupt controller here.
const PRIORITY: usize = 0x0;
const PENDING: usize = 0x1000;

const MENABLE: usize = 0x2000;
const MENABLE_HART: usize = 0x100;
const SENABLE: usize = 0x2080;
const SENABLE_HART: usize = 0x100;
const MPRIORITY: usize = 0x200000;
const MPRIORITY_HART: usize = 0x2000;
const SPRIORITY: usize = 0x201000;
const SPRIORITY_HART: usize = 0x2000;
const MCLAIM: usize = 0x200004;
const MCLAIM_HART: usize = 0x2000;
const SCLAIM: usize = 0x201004;
const SCLAIM_HART: usize = 0x2000;

#[inline]
fn read(offset: usize) -> u32 {
    unsafe {
        let src = (Into::<usize>::into(PLIC) + offset) as *const u32;
        ptr::read_volatile(src)
    }
}

#[inline]
fn write(offset: usize, value: u32) {
    unsafe {
        let dst = (Into::<usize>::into(PLIC) + offset) as *mut u32;
        ptr::write_volatile(dst, value);
    }
}
