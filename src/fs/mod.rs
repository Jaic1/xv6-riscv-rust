use core::cell::Cell;

use crate::consts::fs::NDIRECT;

mod dir;
mod inode;
mod bio;
mod block;

pub use bio::Buf;
pub use bio::BCACHE;

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
    block::init_super_block(dev);
    // TODO - init log
    println!("file system: setup done");
}

