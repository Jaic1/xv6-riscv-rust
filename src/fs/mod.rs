//! File system

use core::cell::Cell;
use core::ops::DerefMut;

use crate::consts::fs::NDIRECT;

mod dir;
mod inode;
mod log;
mod bio;
mod superblock;

pub use bio::Buf;
pub use bio::BCACHE;
pub use log::LOG;

use superblock::SUPER_BLOCK;
use log::Log;
use bio::BufData;

/// On-disk inode structure
#[repr(C)]
struct DInode {
    itype: u16,
    major: u16,
    minor: u16,
    nlink: u16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

/// in-memory copy of an inode
#[repr(C)]
pub struct Inode {
    dev: u32,
    inum: u32,
    iref: u32,
    valid: bool,
    // copy of disk inode
    itype: Cell<u16>,
    major: Cell<u16>,
    minor: Cell<u16>,
    nlink: Cell<u16>,
    size: Cell<u32>,
    addrs: Cell<[u32; NDIRECT + 1]>,
}

impl Inode {
    const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            iref: 0,
            valid: false,
            itype: Cell::new(0),
            major: Cell::new(0),
            minor: Cell::new(0),
            nlink: Cell::new(0),
            size: Cell::new(0),
            addrs: Cell::new([0; NDIRECT + 1]),
        }
    }
}

/// Init fs.
/// Read super block info.
/// Init log info and recover if necessary.
pub unsafe fn init(dev: u32) {
    SUPER_BLOCK.init(dev);
    let log_ptr = LOG.lock().deref_mut() as *mut Log;
    log_ptr.as_mut().unwrap().init(dev);
    println!("file system: setup done");
}

