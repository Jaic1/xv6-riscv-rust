//! file system module

use core::cell::Cell;
use core::option::Option;
use core::ptr;

pub mod bio;
pub mod dir;
pub mod inode;

pub use bio::binit;

use bio::{bread, brelse};
use inode::iget;

pub const BSIZE: usize = 1024;
const NINODE: usize = 50;
const NDIRECT: usize = 12;
const DIRSIZ: usize = 14;
const NBUF: usize = 30;

pub const ROOTDEV: u32 = 1;
const ROOTINO: u32 = 1;
const FSMAGIC: u32 = 0x10203040;

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
    addrs: [Cell<u32>; NDIRECT + 1],
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
            addrs: [Cell::new(0); NDIRECT + 1],
        }
    }
}

pub struct Buf {
    valid: Cell<bool>,
    pub disk: Cell<bool>,
    dev: u32,
    pub blockno: u32,
    refcnt: usize,
    prev: Option<*mut Buf>,
    next: Option<*mut Buf>,
    pub data: [u8; BSIZE],
}

impl Buf {
    const fn new() -> Self {
        Self {
            valid: Cell::new(false),
            disk: Cell::new(false),
            dev: 0,
            blockno: 0,
            refcnt: 0,
            prev: None,
            next: None,
            data: [0; BSIZE],
        }
    }
}

/// Init fs
pub fn init(dev: u32) {
    read_super_block(dev);
    if unsafe { SB.magic } != FSMAGIC {
        panic!("fs::init: invalid file system");
    }
    println!("read file system super block..done");
}

static mut SB: SuperBlock = SuperBlock::new();

/// super block describes the disk layout
#[repr(C)]
struct SuperBlock {
    magic: u32,      // Must be FSMAGIC
    size: u32,       // Size of file system image (blocks)
    nblocks: u32,    // Number of data blocks
    ninodes: u32,    // Number of inodes.
    nlog: u32,       // Number of log blocks
    logstart: u32,   // Block number of first log block
    inodestart: u32, // Block number of first inode block
    bmapstart: u32,  // Block number of first free map block
}

impl SuperBlock {
    const fn new() -> Self {
        Self {
            magic: 0,
            size: 0,
            nblocks: 0,
            ninodes: 0,
            nlog: 0,
            logstart: 0,
            inodestart: 0,
            bmapstart: 0,
        }
    }
}

/// Read the super block
fn read_super_block(dev: u32) {
    let bp = bread(dev, 1);
    unsafe {
        ptr::copy(
            bp.data.as_ptr() as *mut SuperBlock,
            &mut SB as *mut SuperBlock,
            1,
        );
    }
    brelse(bp.dev, bp.blockno);
}
