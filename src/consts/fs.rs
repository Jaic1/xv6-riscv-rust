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
/// maxinum size of directory name, counting 0 in the end
pub const DIRSIZE: usize = 14;

/// root device number
pub const ROOTDEV: u32 = 1;
/// root inode number in root device
/// i.e., starting inode of the file tree structure
pub const ROOTINUM: u32 = 1;
/// root inode path name
pub const ROOTIPATH: [u8; 2] = [b'/', 0];

/// maxinum of blocks an FS op can write
pub const MAXOPBLOCKS: usize = 10;
/// size of buffer cache for block
pub const NBUF: usize = MAXOPBLOCKS * 3;
/// size of log space in disk
pub const LOGSIZE: usize = MAXOPBLOCKS * 3;
