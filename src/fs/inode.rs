//! Inode-relevant operations

use array_macro::array;

use core::{cmp::min, mem, panic, ptr};

use crate::mm::Address;
use crate::spinlock::SpinLock;
use crate::sleeplock::{SleepLock, SleepLockGuard};
use crate::process::CPU_MANAGER;
use crate::consts::fs::{NINODE, BSIZE, NDIRECT, NINDIRECT, MAX_DIR_SIZE, MAX_FILE_SIZE, ROOTDEV, ROOTINUM};
use super::{BCACHE, BufData, superblock::SUPER_BLOCK, LOG};
use super::block::{bm_alloc, bm_free, inode_alloc};

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
        guard[empty_i].inum = inum;
        guard[empty_i].refs = 1;
        Inode {
            dev,
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
            if idata.valid.is_none() || idata.dinode.nlink > 0 {
                idata.valid.take();
                drop(idata);
                imeta.refs -= 1;
                drop(guard);
            } else {
                drop(guard);
                idata.dinode.itype = InodeType::Empty;
                idata.truncate();
                idata.valid.take();
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
    fn namex(&self, path: &[u8], name: &mut [u8; MAX_DIR_SIZE], is_parent: bool) -> Option<Inode> {
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
            match data_guard.dir_lookup(name, false) {
                None => {
                    drop(data_guard);
                    return None
                },
                Some((last_inode, _)) => {
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
        let mut name: [u8; MAX_DIR_SIZE] = [0; MAX_DIR_SIZE];
        self.namex(path, &mut name, false)
    }

    /// Same behavior as `namei`, but return the parent of the inode,
    /// and copy the end path into name.
    pub fn namei_parent(&self, path: &[u8], name: &mut [u8; MAX_DIR_SIZE]) -> Option<Inode> {
        self.namex(path, name, true)
    }

    /// Given the inode path, lookup and create it.
    /// When the inode on the specificed path is already created,
    /// i.e., successfully looked up,
    /// return it or [`None`] according to the reuse flag.
    pub fn create(&self, path: &[u8], itype: InodeType, major: u16, minor: u16, reuse: bool) -> Option<Inode> {
        let mut name: [u8; MAX_DIR_SIZE] = [0; MAX_DIR_SIZE];
        let dir_inode = self.namei_parent(path, &mut name)?;
        let mut dir_idata = dir_inode.lock();

        // lookup first
        if let Some((inode, _)) = dir_idata.dir_lookup(&name, false) {
            if reuse {
                return Some(inode)
            } else {
                return None
            }
        }

        // not found, create it
        let (dev, _) = *dir_idata.valid.as_ref().unwrap();
        let inum = inode_alloc(dev, itype);
        let inode = self.get(dev, inum);
        let mut idata = inode.lock();
        idata.dinode.major = major;
        idata.dinode.minor = minor;
        idata.dinode.nlink = 1;
        idata.update();
        debug_assert_eq!(idata.dinode.itype, itype);

        // if dir, create . and ..
        if itype == InodeType::Directory {
            dir_idata.dinode.nlink += 1;
            dir_idata.update();
            let mut name: [u8; MAX_DIR_SIZE] = [0; MAX_DIR_SIZE];
            // . -> itself
            name[0] = b'.';
            if idata.dir_link(&name, inum).is_err() {
                panic!("dir link .");
            }
            // .. -> parent
            name[1] = b'.';
            if idata.dir_link(&name, dir_inode.inum).is_err() {
                panic!("dir link ..");
            }
        }

        if dir_idata.dir_link(&name, inum).is_err() {
            panic!("parent dir link");
        }

        drop(dir_idata);
        drop(dir_inode);
        drop(idata);
        Some(inode)
    }
}

/// Skip the path starting at cur by b'/'s.
/// It will copy the skipped content to name.
/// Return the current offset after skipping.
fn skip_path(path: &[u8], mut cur: usize, name: &mut [u8; MAX_DIR_SIZE]) -> usize {
    // skip preceding b'/'
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

    // skip succeeding b'/'
    while path[cur] == b'/' {
        cur += 1;
    }
    cur
}

/// Inode handed out by inode cache.
/// It is actually a handle pointing to the cache.
#[derive(Debug)]
pub struct Inode {
    dev: u32,
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

        if guard.valid.is_none() {
            let buf = BCACHE.bread(self.dev, unsafe { SUPER_BLOCK.locate_inode(self.inum) });
            let offset = locate_inode_offset(self.inum);
            let dinode = unsafe { (buf.raw_data() as *const DiskInode).offset(offset) };
            guard.dinode = unsafe { ptr::read(dinode) };
            drop(buf);
            guard.valid = Some((self.dev, self.inum));
            if guard.dinode.itype == InodeType::Empty {
                panic!("inode: lock an empty inode");
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
    /// inode number
    inum: u32,
    /// reference count
    refs: usize,
}

impl InodeMeta {
    const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            refs: 0,
        }
    }
}

#[derive(Debug)]
/// In-memory copy of an inode
pub struct InodeData {
    /// 0: dev, 1: inum
    valid: Option<(u32, u32)>,
    dinode: DiskInode,
}

impl InodeData {
    const fn new() -> Self {
        Self {
            valid: None,
            dinode: DiskInode::new(),
        }
    }

    /// Get inode dev and inum.
    #[inline]
    pub fn get_dev_inum(&self) -> (u32, u32) {
        self.valid.unwrap()
    }

    /// Get inode type.
    #[inline]
    pub fn get_itype(&self) -> InodeType {
        self.dinode.itype
    }

    /// Get device number.
    #[inline]
    pub fn get_devnum(&self) -> (u16, u16) {
        (self.dinode.major, self.dinode.minor)
    }

    /// Increase the hard link by 1.
    #[inline]
    pub fn link(&mut self) {
        self.dinode.nlink += 1;
    }

    /// Decrease the hard link by 1.
    pub fn unlink(&mut self) {
        self.dinode.nlink -= 1;
    }

    /// Discard the inode data/content.
    pub fn truncate(&mut self) {
        let (dev, _) = *self.valid.as_ref().unwrap();

        // direct block
        for i in 0..NDIRECT {
            if self.dinode.addrs[i] > 0 {
                bm_free(dev, self.dinode.addrs[i]);
                self.dinode.addrs[i] = 0;
            }
        }

        // indirect block
        if self.dinode.addrs[NDIRECT] > 0 {
            let buf = BCACHE.bread(dev, self.dinode.addrs[NDIRECT]);
            let buf_ptr = buf.raw_data() as *const BlockNo;
            for i in 0..NINDIRECT {
                let bn = unsafe { ptr::read(buf_ptr.offset(i as isize)) };
                if bn > 0 {
                    bm_free(dev, bn);
                }
            }
            drop(buf);
            bm_free(dev, self.dinode.addrs[NDIRECT]);
            self.dinode.addrs[NDIRECT] = 0;
        }

        self.dinode.size = 0;
        self.update();
    }

    /// Upate a modified in-memory inode to disk.
    /// Typically called after changing the inode info.
    pub fn update(&mut self) {
        let (dev, inum) = *self.valid.as_ref().unwrap();

        let mut buf = BCACHE.bread(dev, unsafe { SUPER_BLOCK.locate_inode(inum) });
        let offset = locate_inode_offset(inum);
        let dinode = unsafe { (buf.raw_data_mut() as *mut DiskInode).offset(offset) };
        unsafe { ptr::write(dinode, self.dinode) };
        LOG.write(buf);
    }

    /// Read inode data from disk.
    /// According to the kind of dst, it will copy to virtual address or kernel address.
    /// Note: `offset` + `count` should not be larger than the data size of inode.
    pub fn iread(&mut self, mut dst: Address, offset: u32, count: u32) -> Result<(), ()> {
        // check the reading content is in range
        let end = offset.checked_add(count).ok_or(())?;
        if end > self.dinode.size {
            return Err(())
        }

        let (dev, _) = *self.valid.as_ref().unwrap();
        let offset = offset as usize;
        let mut count = count as usize;
        let mut block_base = offset / BSIZE;
        let block_offset = offset % BSIZE;
        let mut read_count = min(BSIZE - block_offset, count);
        let mut block_offset = block_offset as isize;
        while count > 0 {
            let buf = BCACHE.bread(dev, self.map_blockno(block_base));
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

    /// Similar to [`iread`].
    /// Try to read as much as possible, return the bytes read.
    pub fn try_iread(&mut self, dst: Address, offset: u32, count: u32) -> Result<u32, ()> {
        // check the reading content is in range
        if offset > self.dinode.size {
            return Ok(0)
        }
        let end = offset.checked_add(count).ok_or(())?;
        let actual_count = if end > self.dinode.size {
            self.dinode.size - offset
        } else {
            count
        };
        self.iread(dst, offset, actual_count)?;
        Ok(actual_count)
    }

    /// Wrapper of [`try_iwrite`].
    /// Succeed only when all the requested count of btyes are written.
    pub fn iwrite(&mut self, src: Address, offset: u32, count: u32) -> Result<(), ()> {
        match self.try_iwrite(src, offset, count) {
            Ok(ret) => if ret == count { Ok(()) } else { Err(()) },
            Err(()) => Err(()),
        }
    }

    /// Try to write inode data to disk as much as possible.
    /// According to the kind of src, it will copy from virtual address or kernel address.
    /// Return the actual bytes written.
    /// Note1: It will automatically increment the size of this inode, i.e.,
    ///     allocate new blocks in the disk/fs, but the offset must be in range.
    pub fn try_iwrite(&mut self, mut src: Address, offset: u32, count: u32) -> Result<u32, ()> {
        // check the writing content is in range
        if offset > self.dinode.size {
            return Err(())
        }
        let end = offset.checked_add(count).ok_or(())? as usize;
        if end > MAX_FILE_SIZE {
            return Err(())
        }

        let (dev, _) = *self.valid.as_ref().unwrap();
        let mut block_base = (offset as usize) / BSIZE;
        let block_offset = (offset as usize) % BSIZE;
        let mut count = count as usize;
        let mut write_count = min(BSIZE - block_offset, count);
        let mut block_offset = block_offset as isize;
        while count > 0 {
            let mut buf = BCACHE.bread(dev, self.map_blockno(block_base));
            let dst_ptr = unsafe { (buf.raw_data_mut() as *mut u8).offset(block_offset) };
            if src.copy_in(dst_ptr, write_count).is_err() {
                break
            };
            LOG.write(buf);

            count -= write_count;
            src = src.offset(write_count);
            block_base += 1;
            block_offset = 0;
            write_count = min(BSIZE, count);
        }

        // end <= MAX_FILE_SIZE <= u32::MAX
        let size = (end - count) as u32;
        if size > self.dinode.size {
            self.dinode.size = size;
        }
        self.update();
        Ok(size-offset)
    }

    /// Give out the inode status.
    pub fn istat(&self, stat: &mut FileStat) {
        let (dev, inum) = self.valid.unwrap();
        stat.dev = dev;
        stat.inum = inum;
        stat.itype = self.dinode.itype;
        stat.nlink = self.dinode.nlink;
        stat.size = self.dinode.size as u64;
    }

    /// Given the relevant nth data block of this inode.
    /// Return the actual (newly in this function call)-allocated blockno in the disk.
    /// Panics if this offset number is out of range.
    fn map_blockno(&mut self, offset_bn: usize) -> u32 {
        let (dev, _) = *self.valid.as_ref().unwrap();
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
            panic!("queried offset_bn out of range");
        }
    }

    /// Look for an inode entry in this directory according the name.
    /// If the `need_offset` flag is set,
    /// also return the corresponding offset of the entry inside the directory.
    /// Panics if this is not a directory.
    fn dir_lookup(&mut self, name: &[u8; MAX_DIR_SIZE], need_offset: bool) -> Option<(Inode, Option<u32>)> {
        let (dev, _) = *self.valid.as_ref().unwrap();
        debug_assert!(dev != 0);
        if self.dinode.itype != InodeType::Directory {
            panic!("inode type not dir");
        }

        let de_size = mem::size_of::<DirEntry>();
        let mut dir_entry = DirEntry::empty();
        let dir_entry_ptr = Address::KernelMut(&mut dir_entry as *mut _ as *mut u8);
        for offset in (0..self.dinode.size).step_by(de_size) {
            self.iread(dir_entry_ptr, offset, de_size as u32).expect("read dir entry");
            if dir_entry.inum == 0 {
                continue;
            }
            for i in 0..MAX_DIR_SIZE {
                if dir_entry.name[i] != name[i] {
                    break;
                }
                if dir_entry.name[i] == 0 {
                    return Some((ICACHE.get(dev, dir_entry.inum as u32),
                        if need_offset { Some(offset) } else { None }))
                }
            }
        }

        None
    }

    /// Write a new [`DirEntry`] into this inode, whose type must be directory.
    /// LTODO - Panics if `inum` is larger than u16::MAX.
    pub fn dir_link(&mut self, name: &[u8; MAX_DIR_SIZE], inum: u32) -> Result<(), ()> {
        if inum > u16::MAX as u32 {
            panic!("inum {} too large", inum);
        }
        let inum = inum as u16;

        // the entry should not be present
        if self.dir_lookup(name, false).is_some() {
            // auto drop the returned inode
            return Err(())
        }

        // allocate a dir entry
        let de_size = mem::size_of::<DirEntry>() as u32;
        let mut dir_entry = DirEntry::empty();
        let dir_entry_ptr = Address::KernelMut(&mut dir_entry as *mut _ as *mut u8);
        let mut offset = self.dinode.size;
        for off in (0..self.dinode.size).step_by(de_size as usize) {
            self.iread(dir_entry_ptr, off, de_size).expect("read dir entry");
            if dir_entry.inum == 0 {
                offset = off;
                break
            }
        }

        assert_eq!(offset % de_size, 0);
        dir_entry.name.copy_from_slice(name);
        dir_entry.inum = inum;
        let dir_entry_ptr = Address::Kernel(&dir_entry as *const _ as *const u8);
        if self.iwrite(dir_entry_ptr, offset, de_size).is_err() {
            panic!("inode write error");
        }

        Ok(())
    }

    /// Unlink an inode according to the name in the current directory.
    /// Also remove its entry in the directory.
    /// Panics if the inode data is not directory.
    /// It must be called within a log transaction.
    pub fn dir_unlink(&mut self, name: &[u8; MAX_DIR_SIZE]) -> Result<(), ()> {
        // the name should not be . and ..
        if name[0] == b'.' && (name[1] == 0 || (name[1] == b'.' && name[2] == 0)) {
            return Err(())
        }

        // lookup the entry correspond to the name
        let inode: Inode;
        let offset: u32;
        match self.dir_lookup(&name, true) {
            Some((i, Some(off))) => {
                inode = i;
                offset = off;
            },
            _ => return Err(()),
        }

        // check the entry
        let mut idata = inode.lock();
        if idata.dinode.nlink < 1 {
            panic!("entry inode's link is zero");
        }
        if idata.dinode.itype == InodeType::Directory && !idata.dir_is_empty() {
            return Err(())
        }

        // empty the entry
        let de_size = mem::size_of::<DirEntry>() as u32;
        let dir_entry = DirEntry::empty();
        let dir_entry_ptr = Address::Kernel(&dir_entry as *const DirEntry as *const u8);
        if self.iwrite(dir_entry_ptr, offset, de_size).is_err() {
            panic!("cannot write entry previously read");
        }

        // decrement some links
        if idata.dinode.itype == InodeType::Directory {
            self.dinode.nlink -= 1;
            self.update();
        }
        idata.dinode.nlink -= 1;
        idata.update();
        
        Ok(())
    }

    /// Test if the directory inode is empty.
    fn dir_is_empty(&mut self) -> bool {
        let de_size = mem::size_of::<DirEntry>() as u32;
        let mut dir_entry = DirEntry::empty();
        let dir_entry_ptr = &mut dir_entry as *mut DirEntry;
        let dir_entry_addr = Address::KernelMut(dir_entry_ptr as *mut u8);
        for offset in ((2*de_size)..(self.dinode.size)).step_by(de_size as usize) {
            if self.iread(dir_entry_addr, offset, de_size).is_err() {
                panic!("read dir entry");
            }
            if dir_entry.inum != 0 {
                return false
            }
        }

        return true
    }
}

/// Number of inodes in a single block.
pub const IPB: usize = BSIZE / mem::size_of::<DiskInode>();

/// Given an inode number.
/// Calculate the offset index of this inode inside the block. 
#[inline]
pub fn locate_inode_offset(inum: u32) -> isize {
    (inum as usize % IPB) as isize
}

/// Check several requirements that inode struct should satisify.
pub fn icheck() {
    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<DiskInode>(), 0);

    // LTODO - replace some u32 to type alias BlockNo
    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<BlockNo>(), 0);
    debug_assert_eq!(mem::size_of::<BlockNo>(), mem::size_of::<u32>());
    debug_assert_eq!(mem::align_of::<BlockNo>(), mem::align_of::<u32>());

    debug_assert_eq!(mem::align_of::<BufData>() % mem::align_of::<DirEntry>(), 0);

    debug_assert!(MAX_FILE_SIZE <= u32::MAX as usize);
}

type BlockNo = u32;

#[repr(C)]
#[derive(Debug)]
pub struct FileStat {
    dev: u32,
    inum: u32,
    itype: InodeType,
    nlink: u16,
    size: u64,
}

impl FileStat {
    pub const fn uninit() -> Self {
        Self {
            dev: 0,
            inum: 0,
            itype: InodeType::Empty,
            nlink: 0,
            size: 0,
        }
    }
}

/// On-disk inode structure
#[repr(C)]
#[derive(Clone, Copy, Debug)]
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

    /// If the [`DiskInode`] is free, i.e., its type is [`InodeType::Empty`],
    /// allocate it by setting its itype.
    pub fn try_alloc(&mut self, itype: InodeType) -> Result<(), ()> {
        if self.itype == InodeType::Empty {
            unsafe { ptr::write_bytes(self, 0, 1); }
            self.itype = itype;
            Ok(())
        } else {
            Err(())
        }
    }
}

/// Inode type.
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InodeType {
    Empty = 0,
    Directory = 1,
    File = 2,
    Device = 3,
}

/// Directory entry in the disk.
#[repr(C)]
struct DirEntry {
    inum: u16,
    name: [u8; MAX_DIR_SIZE],
}

impl DirEntry {
    const fn empty() -> Self {
        Self {
            inum: 0,
            name: [0; MAX_DIR_SIZE],
        }
    }
}
