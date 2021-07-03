/// magic number indentifying this specific file system
pub const FSMAGIC: u32 = 0x10203040;
/// size of disk block
pub const BSIZE: usize = 1024;

/// number of bits per bitmap block
pub const BPB: u32 = (BSIZE * 8) as u32;

/// number of inodes in inode cache
pub const NINODE: usize = 50;
pub const NDIRECT: usize = 12;
/// number of indirect blocks in a single block
/// note: the blockno should be u32
pub const NINDIRECT: usize = BSIZE / core::mem::size_of::<u32>();
pub const DIRSIZ: usize = 14;

pub const ROOTDEV: u32 = 1;
pub const ROOTINO: u32 = 1;

/// maxinum of blocks an FS op can write
pub const MAXOPBLOCKS: usize = 10;
/// size of buffer cache for block
pub const NBUF: usize = MAXOPBLOCKS * 3;
/// size of log space in disk
pub const LOGSIZE: usize = MAXOPBLOCKS * 3;
