//! Inode-relevant operations

use core::{mem, panic, ptr};

use array_macro::array;

use crate::{consts::fs::NINDIRECT, sleeplock::SleepLockGuard, spinlock::SpinLock};
use crate::sleeplock::SleepLock;
use crate::consts::fs::{NINODE, BSIZE, NDIRECT};
use super::{BCACHE, BufData, bitmap::bm_free, superblock::SUPER_BLOCK, LOG};

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
    /// It should only be called by the Drop impl of Inode.
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
    dinode: DiskInode,
}

impl InodeData {
    const fn new() -> Self {
        Self {
            valid: false,
            dinode: DiskInode::new(),
        }
    }

    /// Discard the inode data/content.
    fn truncate(&mut self, inode: &Inode) {
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
    fn update(&mut self, inode: &Inode) {
        let mut buf = BCACHE.bread(inode.dev, inode.blockno);
        let offset = locate_inode_offset(inode.inum) as isize;
        let dinode = unsafe { (buf.raw_data_mut() as *mut DiskInode).offset(offset) };
        unsafe { ptr::write(dinode, self.dinode) };
        LOG.write(buf);
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
    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<BlockNo>(), 0);
    // LTODO - replace some u32 to type alias BlockNo
    debug_assert_eq!(mem::size_of::<BlockNo>(), mem::size_of::<u32>());
    debug_assert_eq!(mem::align_of::<BlockNo>(), mem::align_of::<u32>());
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
    /// Size of data of this inode.
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
