//! Physical memory layout
//!
//! qemu -machine virt is set up like this,
//! based on qemu's hw/riscv/virt.c:
//!
//! 00001000 -- boot ROM, provided by qemu
//! 02000000 -- CLINT
//! 0C000000 -- PLIC
//! 10000000 -- uart0
//! 10001000 -- virtio disk
//! 80000000 -- boot ROM jumps here in machine mode
//!             -kernel loads the kernel here
//! unused RAM after 80000000.
//!
//! the kernel uses physical memory thus:
//! 80000000 -- entry.S, then kernel text and data
//! end -- start of kernel page allocation area
//! PHYSTOP -- end RAM used by the kernel

use crate::consts::PGSIZE;

// local interrupt controller, which contains the timer.
pub const CLINT: usize = 0x2000000;
pub const CLINT_MAP_SIZE: usize = 0x10000;
pub const CLINT_MTIMECMP: usize = CLINT + 0x4000;
pub const CLINT_MTIME: usize = CLINT + 0xbff8;

// qemu puts UART registers here in physical memory.
pub const UART0: usize = 0x10000000;
pub const UART0_MAP_SIZE: usize = PGSIZE;

// virtio mmio interface
pub const VIRTIO0: usize = 0x10001000;
pub const VIRTIO0_MAP_SIZE: usize = PGSIZE;

// qemu puts programmable interrupt controller here.
pub const PLIC: usize = 0x0c000000;
pub const PLIC_MAP_SIZE: usize = 0x400000;

// the kernel expects there to be RAM
// for use by the kernel and user pages
// from physical address 0x80000000 to PHYSTOP.
pub const KERNBASE: usize = 0x80000000;
pub const PHYSTOP: usize = KERNBASE + 128 * 1024 * 1024;
