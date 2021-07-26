use core::sync::atomic::AtomicBool;

use crate::{consts::driver::NDEV, mm::Address};

pub mod virtio_disk;
pub mod console;
pub mod uart;

/// Used to signal whether any of the harts panic.
pub(crate) static PANICKED: AtomicBool = AtomicBool::new(false);

pub static DEVICES: [Option<Device>; NDEV] = [
    /* 0 */   None,
    /* 1 */   Some(Device { read: console::read, write: console::write }),
    /* 2 */   None,
    /* 3 */   None,
    /* 4 */   None,
    /* 5 */   None,
    /* 6 */   None,
    /* 7 */   None,
    /* 8 */   None,
    /* 9 */   None,
];

pub struct Device {
    /// function: read from [`Address`] count bytes.
    pub read: fn(Address, u32) -> Result<u32, ()>,
    /// function: write to [`Address`] count bytes.
    pub write: fn(Address, u32) -> Result<u32, ()>,
}
