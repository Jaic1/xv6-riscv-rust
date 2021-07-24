//! disk block operations
//! include bitmap and inode block

use core::ptr;

use bit_field::BitField;

use crate::consts::fs::BPB;

use super::{BCACHE, superblock::SUPER_BLOCK, LOG};
use super::inode::{DiskInode, InodeType, locate_inode_offset};

/// Allocate a free block in the disk/fs.
/// It will zero the block content before return it.
/// Panics if it cannot find any available free block.
pub fn bm_alloc(dev: u32) -> u32 {
    // first, iterate each bitmap block
    let total_block = unsafe { SUPER_BLOCK.size() };
    for base in (0..total_block).step_by(BPB as usize) {
        let mut buf = BCACHE.bread(dev, unsafe { SUPER_BLOCK.bitmap_blockno(base) });
        // second, iterate each bit in the bitmap block
        for offset in 0..BPB {
            if base + offset >= total_block {
                break;
            }
            let index = (offset / 8) as isize;
            let bit = (offset % 8) as usize;
            let byte = unsafe { (buf.raw_data_mut() as *mut u8).offset(index).as_mut().unwrap() };
            if byte.get_bit(bit) {
                continue;
            }
            byte.set_bit(bit, true);
            LOG.write(buf);

            // zero the free block
            let free_bn = base + offset;
            let mut free_buf = BCACHE.bread(dev, free_bn);
            unsafe { ptr::write_bytes(free_buf.raw_data_mut(), 0, 1); }
            LOG.write(free_buf);
            return free_bn
        }
        drop(buf);
    }

    panic!("bitmap: cannot alloc any free block");
}

/// Free a block in the disk by setting the relevant bit in bitmap to 0. 
pub fn bm_free(dev: u32, blockno: u32) {
    let bm_blockno = unsafe { SUPER_BLOCK.bitmap_blockno(blockno) };
    let bm_offset = blockno % BPB;
    let index = (bm_offset / 8) as isize;
    let bit = (bm_offset % 8) as usize;
    let mut buf = BCACHE.bread(dev, bm_blockno);
    
    let byte = unsafe { (buf.raw_data_mut() as *mut u8).offset(index).as_mut().unwrap() };
    if !byte.get_bit(bit) {
        panic!("bitmap: double freeing a block");
    }
    byte.set_bit(bit, false);
    LOG.write(buf);
}

/// Allocate an inode in the disk/fs, return the inode number.
/// Panics if there are not enough inodes.
pub fn inode_alloc(dev: u32, itype: InodeType) -> u32 {
    let size = unsafe { SUPER_BLOCK.inode_size() };
    for inum in 1..size {
        let blockno = unsafe { SUPER_BLOCK.locate_inode(inum) };
        let offset = locate_inode_offset(inum);
        let mut buf = BCACHE.bread(dev, blockno);
        let dinode = unsafe { (buf.raw_data_mut() as *mut DiskInode).offset(offset) };
        let dinode = unsafe { &mut *dinode };
        if dinode.try_alloc(itype).is_ok() {
            LOG.write(buf);
            return inum
        }
    }

    panic!("not enough inode to alloc");
}
