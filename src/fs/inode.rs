//! Inode-relevant operations

use core::{cmp::min, mem, panic, ptr};

use array_macro::array;

use crate::{consts::fs::NINDIRECT, mm::Address, sleeplock::SleepLockGuard, spinlock::SpinLock};
use crate::sleeplock::SleepLock;
use crate::process::CPU_MANAGER;
use crate::consts::fs::{NINODE, BSIZE, NDIRECT, DIRSIZE, ROOTDEV, ROOTINUM};
use super::{BCACHE, BufData, superblock::SUPER_BLOCK, LOG};
use super::bitmap::{bm_alloc, bm_free};

pub static ICACHE: InodeCache = InodeCache::new();

pub struct InodeCache {
    meta: SpinLock<[InodeMeta; NINODE]>,
    data: [SleepLock<InodeData>; NINODE],
}

impl InodeCache {
    const fn new() -> Self {
        Self {
            meta: SpinLock::new(array![_ => InodeMeta::new(); NINODE], "InodeMeta"),
            data: array![_ => SleepLock::new(InodeData::new(), "InodeData"); NINODE],
        }
    }

    /// Lookup the inode in the inode cache.
    /// If found, return an handle.
    /// If not found, alloc an in-memory location in the cache,
    ///     but not fetch it from the disk yet.
    fn get(&self, dev: u32, inum: u32) -> Inode {
        let mut guard = self.meta.lock();
        
        // lookup in the cache
        let mut empty_i: Option<usize> = None;
        for i in 0..NINODE {
            if guard[i].inum == inum && guard[i].refs > 0 && guard[i].dev == dev {
                guard[i].refs += 1;
                return Inode { 
                    dev,
                    blockno: guard[i].blockno,
                    inum,
                    index: i,
                }
            }
            if empty_i.is_none() && guard[i].refs == 0 {
                empty_i = Some(i);
            }
        }

        // not found
        let empty_i = match empty_i {
            Some(i) => i,
            None => panic!("inode: not enough"),
        };
        guard[empty_i].dev = dev;
        let blockno = unsafe { SUPER_BLOCK.locate_inode(inum) };
        guard[empty_i].blockno = blockno;
        guard[empty_i].inum = inum;
        guard[empty_i].refs = 1;
        Inode {
            dev,
            blockno,
            inum,
            index: empty_i
        }
    }

    /// Clone an inode by just increment its reference count by 1.
    fn dup(&self, inode: &Inode) -> Inode {
        let mut guard = self.meta.lock();
        guard[inode.index].refs += 1;
        Inode {
            dev: inode.dev,
            blockno: inode.blockno,
            inum: inode.inum,
            index: inode.index,
        }
    }

    /// Done with this inode.
    /// If this is the last reference in the inode cache, then is might be recycled.
    /// Further, if this inode has no links anymore, free this inode in the disk.
    /// It should only be called by the Drop impl of [`Inode`].
    fn put(&self, inode: &mut Inode) {
        let mut guard = self.meta.lock();
        let i = inode.index;
        let imeta = &mut guard[i];

        if imeta.refs == 1 {
            // SAFETY: reference count is 1, so this lock will not block.
            let mut idata = self.data[i].lock();
            if !idata.valid || idata.dinode.nlink > 0 {
                drop(idata);
                imeta.refs -= 1;
                drop(guard);
            } else {
                drop(guard);
                idata.dinode.itype = InodeType::Empty;
                idata.truncate(inode);
                idata.dev = 0;
                idata.valid = false;
                drop(idata);

                // recycle after this inode content in the cache is no longer valid.
                // note: it is wrong to recycle it earlier,
                // otherwise the cache content might change
                // before the previous content written to disk.
                let mut guard = self.meta.lock();
                guard[i].refs -= 1;
                debug_assert_eq!(guard[i].refs, 0);
                drop(guard);
            }
        } else {
            imeta.refs -= 1;
            drop(guard);
        }
    }

    /// Helper function for `namei` and `namei_parent`.
    fn namex(&self, path: &[u8], name: &mut [u8; DIRSIZE], is_parent: bool) -> Option<Inode> {
        let mut inode: Inode;
        if path[0] == b'/' {
            inode = self.get(ROOTDEV, ROOTINUM);
        } else {
            let p = unsafe { CPU_MANAGER.my_proc() };
            inode = self.dup(p.data.get_mut().cwd.as_ref().unwrap());
        }

        let mut cur: usize = 0;
        loop {
            cur = skip_path(path, cur, name);
            if cur == 0 {
                break;
            }
            let mut data_guard = inode.lock();
            if data_guard.dinode.itype != InodeType::Directory {
                drop(data_guard);
                return None
            }
            if is_parent && path[cur] == 0 {
                drop(data_guard);
                return Some(inode)
            }
            match data_guard.dir_lookup(name) {
                None => {
                    drop(data_guard);
                    return None
                },
                Some(last_inode) => {
                    drop(data_guard);
                    inode = last_inode;
                },
            }
        }

        if is_parent {
            // only when querying root inode's parent
            println!("kernel warning: namex querying root inode's parent");
            None
        } else {
            Some(inode)
        }
    }

    /// namei interprets the path argument as an pathname to Unix file.
    /// It will return an [`Inode`] if succeed, Err(()) if fail.
    /// It must be called inside a transaction(i.e., `begin_op` and `end_op`) since it calls `put`.
    /// Note: the path should end with 0u8, otherwise it might panic due to out-of-bound.
    pub fn namei(&self, path: &[u8]) -> Option<Inode> {
        let mut name: [u8; DIRSIZE] = [0; DIRSIZE];
        self.namex(path, &mut name, false)
    }

    /// Same behavior as `namei`, but return the parent of the inode,
    /// and copy the end path into name.
    pub fn namei_parent(&self, path: &[u8], name: &mut [u8; DIRSIZE]) -> Option<Inode> {
        self.namex(path, name, true)
    }
}

/// Skip the path starting at cur by '/'s.
/// It will copy the skipped content to name.
/// Return the current offset after skipping.
fn skip_path(path: &[u8], mut cur: usize, name: &mut [u8; DIRSIZE]) -> usize {
    // skip preceding '/'
    while path[cur] == b'/' {
        cur += 1;
    }
    if path[cur] == 0 {
        return 0
    }

    let start = cur;
    while path[cur] != b'/' && path[cur] != 0 {
        cur += 1;
    }
    let mut count = cur - start;
    if count >= name.len() {
        debug_assert!(false);
        count = name.len() - 1;
    }
    unsafe { ptr::copy(path.as_ptr().offset(start as isize), name.as_mut_ptr(), count); }
    name[count] = 0;

    // skip succeeding '/'
    while path[cur] == b'/' {
        cur += 1;
    }
    cur
}

/// Inode handed out by inode cache.
/// It is actually a handle pointing to the cache.
pub struct Inode {
    dev: u32,
    blockno: u32,
    inum: u32,
    index: usize,
}

impl Clone for Inode {
    fn clone(&self) -> Self {
        ICACHE.dup(self)
    }
}

impl Inode {
    /// Lock the inode.
    /// Load it from the disk if its content not cached yet.
    pub fn lock<'a>(&'a self) -> SleepLockGuard<'a, InodeData> {
        let mut guard = ICACHE.data[self.index].lock();

        if !guard.valid {
            guard.dev = self.dev;
            let buf = BCACHE.bread(self.dev, self.blockno);
            let offset = locate_inode_offset(self.inum) as isize;
            let dinode = unsafe { (buf.raw_data() as *const DiskInode).offset(offset) };
            guard.dinode = unsafe { ptr::read(dinode) };
            drop(buf);
            guard.valid = true;
            if guard.dinode.itype == InodeType::Empty {
                panic!("inode: trying to lock an inode whose type is empty");
            }
        }

        guard
    }
}

impl Drop for Inode {
    /// Done with this inode.
    /// If this is the last reference in the inode cache, then is might be recycled.
    /// Further, if this inode has no links anymore, free this inode in the disk.
    fn drop(&mut self) {
        ICACHE.put(self);
    }
}

struct InodeMeta {
    /// device number
    dev: u32,
    /// block number, calculated from inum
    blockno: u32,
    /// inode number
    inum: u32,
    /// reference count
    refs: usize,
}

impl InodeMeta {
    const fn new() -> Self {
        Self {
            dev: 0,
            blockno: 0,
            inum: 0,
            refs: 0,
        }
    }
}

/// In-memory copy of an inode
pub struct InodeData {
    valid: bool,
    dev: u32,
    dinode: DiskInode,
}

impl InodeData {
    const fn new() -> Self {
        Self {
            valid: false,
            dev: 0,
            dinode: DiskInode::new(),
        }
    }

    /// Discard the inode data/content.
    pub fn truncate(&mut self, inode: &Inode) {
        // direct block
        for i in 0..NDIRECT {
            if self.dinode.addrs[i] > 0 {
                bm_free(inode.dev, self.dinode.addrs[i]);
                self.dinode.addrs[i] = 0;
            }
        }

        // indirect block
        if self.dinode.addrs[NDIRECT] > 0 {
            let buf = BCACHE.bread(inode.dev, self.dinode.addrs[NDIRECT]);
            let buf_ptr = buf.raw_data() as *const BlockNo;
            for i in 0..NINDIRECT {
                let bn = unsafe { ptr::read(buf_ptr.offset(i as isize)) };
                if bn > 0 {
                    bm_free(inode.dev, bn);
                }
            }
            drop(buf);
            bm_free(inode.dev, self.dinode.addrs[NDIRECT]);
            self.dinode.addrs[NDIRECT] = 0;
        }

        self.dinode.size = 0;
        self.update(inode);
    }

    /// Upate a modified in-memory inode to disk.
    /// Typically called after changing the content of inode info.
    pub fn update(&mut self, inode: &Inode) {
        let mut buf = BCACHE.bread(inode.dev, inode.blockno);
        let offset = locate_inode_offset(inode.inum) as isize;
        let dinode = unsafe { (buf.raw_data_mut() as *mut DiskInode).offset(offset) };
        unsafe { ptr::write(dinode, self.dinode) };
        LOG.write(buf);
    }

    /// Read inode data from disk.
    /// According to the kind of dst, it will copy to virtual address or kernel address.
    pub fn read(&mut self, mut dst: Address, offset: u32, count: u32) -> Result<(), ()> {
        // check the reading content is in range
        let end = offset.checked_add(count).ok_or(())?;
        if end > self.dinode.size {
            return Err(())
        }

        let offset = offset as usize;
        let mut count = count as usize;
        let mut block_base = offset / BSIZE;
        let block_offset = offset % BSIZE;
        let mut read_count = min(BSIZE - block_offset, count);
        let mut block_offset = block_offset as isize;
        while count > 0 {
            let buf = BCACHE.bread(self.dev, self.map_blockno(self.dev, block_base));
            let src_ptr = unsafe { (buf.raw_data() as *const u8).offset(block_offset) };
            dst.copy_out(src_ptr, read_count)?;
            drop(buf);

            count -= read_count;
            dst = dst.offset(read_count);
            block_base += 1;
            block_offset = 0;
            read_count = min(BSIZE, count);
        }
        Ok(())
    }

    // LTODO - try_read? for fileread

    /// Write inode data to disk.
    /// According to the kind of src, it will copy from virtual address or kernel address.
    pub fn write(&mut self, mut src: Address, offset: u32, count: u32) -> Result<(), ()> {
        // check the writing content is in range
        let end = offset.checked_add(count).ok_or(())?;
        if end > self.dinode.size {
            return Err(())
        }

        let offset = offset as usize;
        let mut count = count as usize;
        let mut block_base = offset / BSIZE;
        let block_offset = offset % BSIZE;
        let mut write_count = min(BSIZE - block_offset, count);
        let mut block_offset = block_offset as isize;
        while count > 0 {
            let mut buf = BCACHE.bread(self.dev, self.map_blockno(self.dev, block_base));
            let dst_ptr = unsafe { (buf.raw_data_mut() as *mut u8).offset(block_offset) };
            src.copy_in(dst_ptr, write_count)?;
            drop(buf);

            count -= write_count;
            src = src.offset(write_count);
            block_base += 1;
            block_offset = 0;
            write_count = min(BSIZE, count);
        }
        Ok(())
    }

    /// Given the relevant nth data block of this inode.
    /// Return the actual (newly in this function call)-allocated blockno in the disk.
    /// Panics if this offset number is out of range.
    fn map_blockno(&mut self, dev: u32, offset_bn: usize) -> u32 {
        if offset_bn < NDIRECT {
            // in direct block
            if self.dinode.addrs[offset_bn] == 0 {
                let free_bn = bm_alloc(dev);
                self.dinode.addrs[offset_bn] = free_bn;
                free_bn
            } else {
                self.dinode.addrs[offset_bn]
            }
        } else if offset_bn < NDIRECT + NINDIRECT {
            // in indirect block
            let count = (offset_bn - NDIRECT) as isize;

            let indirect_bn = if self.dinode.addrs[NDIRECT] == 0 {
                let free_bn = bm_alloc(dev);
                self.dinode.addrs[NDIRECT] = free_bn;
                free_bn
            } else {
                self.dinode.addrs[NDIRECT]
            };
            let mut indirect_buf = BCACHE.bread(dev, indirect_bn);
            let bn_ptr = unsafe { (indirect_buf.raw_data_mut() as *mut BlockNo).offset(count) };
            let bn = unsafe { ptr::read(bn_ptr) };
            if bn == 0 {
                let free_bn = bm_alloc(dev);
                unsafe { ptr::write(bn_ptr, free_bn); }
                LOG.write(indirect_buf);
                free_bn
            } else {
                drop(indirect_buf);
                bn
            }
        } else {
            panic!("inode: queried offset blockno is out of range");
        }
    }

    /// Look for an inode entry in this directory according the name.
    /// Panics if this is not a directory.
    fn dir_lookup(&mut self, name: &[u8; DIRSIZE]) -> Option<Inode> {
        debug_assert!(self.dev != 0);
        if self.dinode.itype != InodeType::Directory {
            panic!("inode type is not directory");
        }

        let de_size = mem::size_of::<DirEntry>();
        let mut dir_entry = DirEntry::new();
        let dir_entry_ptr = Address::KernelMut(&mut dir_entry as *mut _ as *mut u8);
        for offset in (0..self.dinode.size).step_by(de_size) {
            self.read(dir_entry_ptr, offset, de_size as u32).expect("cannot read entry in this dir");
            if dir_entry.inum == 0 {
                continue;
            }
            for i in 0..DIRSIZE {
                if dir_entry.name[i] != name[i] {
                    break;
                }
                if dir_entry.name[i] == 0 {
                    return Some(ICACHE.get(self.dev, dir_entry.inum as u32))
                }
            }
        }

        None
    }
}

/// Number of inodes in a single block.
pub const IPB: usize = BSIZE / mem::size_of::<DiskInode>();

/// Given an inode number.
/// Calculate the offset index of this inode inside the block. 
#[inline]
fn locate_inode_offset(inum: u32) -> usize {
    inum as usize % IPB
}

/// Check several requirements that inode struct should satisify.
pub fn icheck() {
    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<DiskInode>(), 0);

    // LTODO - replace some u32 to type alias BlockNo
    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<BlockNo>(), 0);
    debug_assert_eq!(mem::size_of::<BlockNo>(), mem::size_of::<u32>());
    debug_assert_eq!(mem::align_of::<BlockNo>(), mem::align_of::<u32>());

    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<DirEntry>(), 0);
}

type BlockNo = u32;

/// On-disk inode structure
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DiskInode {
    /// File type.
    /// 0: empty, 1: file, 2: dir, 3: device 
    itype: InodeType,
    /// Major device number, for device only.
    major: u16,
    /// Minor device number, for device only.
    minor: u16,
    /// Hard links to this inode.
    nlink: u16,
    /// Size of actual data/content of this inode.
    size: u32,
    /// Data address.
    addrs: [u32; NDIRECT + 1],
}

impl DiskInode {
    const fn new() -> Self {
        Self {
            itype: InodeType::Empty,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT + 1],
        }
    }
}

#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum InodeType {
    Empty = 0,
    File = 1,
    Directory = 2,
    Device = 3,
}

#[repr(C)]
struct DirEntry {
    inum: u16,
    name: [u8; DIRSIZE],
}

impl DirEntry {
    const fn new() -> Self {
        Self {
            inum: 0,
            name: [0; DIRSIZE],
        }
    }
}

// static mut ICACHE: Icache = Icache::new();

// struct Icache {
//     lock: SpinLock<()>,
//     inodes: [Inode; NINODE],
// }

// impl Icache {
//     const fn new() -> Self {
//         Self {
//             lock: SpinLock::new((), "icache"),
//             inodes: array![_ => Inode::new(); NINODE],
//         }
//     }
// }

// /// Find the inode with number inum on device dev
// /// and return the in-memory copy. Does not lock
// /// the inode and does not read it from disk.
// pub fn iget(dev: u32, inum: u32) -> &'static Inode {
//     let icache = unsafe {ICACHE.lock.lock()};

//     // Is the inode we are looking for already cached?
//     let mut empty: Option<&mut Inode> = None;
//     for ip in unsafe {ICACHE.inodes.iter_mut()} {
//         if ip.iref > 0 && ip.dev == dev && ip.inum == inum {
//             ip.iref += 1;
//             drop(icache);
//             return ip;
//         }
//         if empty.is_none() && ip.iref == 0 {
//             empty = Some(ip);
//         }
//     }

//     // Recycle an inode cacahe entry
//     if empty.is_none() {
//         panic!("iget: no enough space in inode cache");
//     }
//     let ip: &mut Inode = empty.take().unwrap();
//     ip.dev = dev;
//     ip.inum = inum;
//     ip.iref = 1;
//     ip.valid = false;
//     drop(icache);
//     ip
// }

// /// Lock the given inode.
// /// Reads the inode from disk if necessary.
// /// LTODO - do not lock the inode yet, only process zero exists
// pub fn ilock(ip: &mut Inode) {
//     if ip.iref < 1 {
//         panic!("ilock: iref smaller than 1");
//     }

//     // acquire sleep lock

//     if !ip.valid {
//         // bp = bread(ip.dev, )
//     }
// }
