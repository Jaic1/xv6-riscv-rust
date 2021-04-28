use core::{mem::MaybeUninit, ptr};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::consts::fs::FSMAGIC;

use super::BCACHE;

static mut SUPER_BLOCK: SuperBlock = SuperBlock::uninit();

/// In-memory copy of superblock
struct SuperBlock {
    data: MaybeUninit<RawSuperBlock>,
    initialized: AtomicBool,
}

impl SuperBlock {
    const fn uninit() -> Self {
        Self {
            data: MaybeUninit::uninit(),
            initialized: AtomicBool::new(false),
        }
    }
}

/// Raw super block describes the disk layout.
#[repr(C)]
struct RawSuperBlock {
    magic: u32,      // Must be FSMAGIC
    size: u32,       // Size of file system image (blocks)
    nblocks: u32,    // Number of data blocks
    ninodes: u32,    // Number of inodes
    nlog: u32,       // Number of log blocks
    logstart: u32,   // Block number of first log block
    inodestart: u32, // Block number of first inode block
    bmapstart: u32,  // Block number of first free map block
}

/// Read the super block from disk into memory.
pub unsafe fn init_super_block(dev: u32) {
    let mut buf = BCACHE.bread(dev, 1);
    ptr::copy(
        buf.raw_data() as *mut RawSuperBlock,
        SUPER_BLOCK.data.as_mut_ptr(),
        1,
    );
    if (*SUPER_BLOCK.data.as_ptr()).magic != FSMAGIC {
        panic!("invalid file system magic num");
    }
    SUPER_BLOCK.initialized.store(true, Ordering::SeqCst);
    drop(buf);
}
