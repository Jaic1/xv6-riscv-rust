//! Bitmap block operations

use bit_field::BitField;

use crate::consts::fs::BPB;
use super::{BCACHE, superblock::SUPER_BLOCK, LOG};

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
