use alloc::sync::Arc;
use core::cmp::min;
use core::convert::TryInto;

use crate::consts::driver::NDEV;
use crate::consts::fs::{MAXOPBLOCKS, BSIZE};
use crate::consts::fs::{O_RDONLY, O_WRONLY, O_RDWR, O_CREATE, O_TRUNC};
use crate::driver::DEVICES;
use crate::mm::Address;

use super::{ICACHE, LOG, inode::FileStat};
use super::{Inode, InodeType};

/// File abstraction above inode.
/// It can represent regular file, device and pipe.
#[derive(Debug)]
pub struct File {
    inner: FileInner,
    readable: bool,
    writable: bool,
}

impl File {
    /// Open a file and optionally create a regular file.
    /// LTODO - avoid stack allocation
    pub fn open(path: &[u8], flags: i32) -> Option<Arc<Self>> {
        LOG.begin_op();

        let inode: Inode;
        if flags & O_CREATE > 0 {
            match ICACHE.create(&path, InodeType::File, 0, 0, true) {
                Some(i) => inode = i,
                None => {
                    LOG.end_op();
                    return None
                }
            }
        } else {
            match ICACHE.namei(&path) {
                Some(i) => inode = i,
                None => {
                    LOG.end_op();
                    return None
                }
            }
        }

        let mut idata = inode.lock();
        let inner;
        let readable = (flags & O_WRONLY) == 0;
        let writable = ((flags & O_WRONLY) | (flags & O_RDWR)) > 0;
        match idata.get_itype() {
            InodeType::Empty => panic!("empty inode"),
            InodeType::Directory => {
                if flags != O_RDONLY {
                    drop(idata); drop(inode); LOG.end_op();
                    return None
                }
                drop(idata);
                inner = FileInner::Regular(FileRegular { offset: 0, inode: Some(inode) });
            },
            InodeType::File => {
                if flags & O_TRUNC > 0 {
                    idata.truncate();
                }
                drop(idata);
                inner = FileInner::Regular(FileRegular { offset: 0, inode: Some(inode) });
            },
            InodeType::Device => {
                let (major, _) = idata.get_devnum();
                if major as usize >= NDEV {
                    drop(idata); drop(inode); LOG.end_op();
                    return None
                }
                drop(idata);
                inner = FileInner::Device(FileDevice { major, inode: Some(inode) });
            }
        }

        LOG.end_op();
        Some(Arc::new(File {
            inner,
            readable,
            writable
        }))
    }

    /// Read from file to user buffer at `addr` in total `count` bytes.
    /// Return the acutal conut of bytes read.
    pub fn fread(&self, addr: Address, count: usize) -> Result<usize, ()> {
        if !self.readable {
            return Err(())
        }

        match self.inner {
            FileInner::Pipe => todo!("pipe read"),
            FileInner::Regular(ref file) => {
                let mut idata = file.inode.as_ref().unwrap().lock();
                match idata.try_iread(addr, file.offset, count.try_into().unwrap()) {
                    Ok(read_count) => {
                        // file.offset += read_count; TODO
                        Ok(read_count as usize)
                    },
                    Err(()) => Err(())
                }
            },
            FileInner::Device(ref dev) => {
                let dev_read = DEVICES[dev.major as usize].as_ref().ok_or(())?.read;
                dev_read(addr, count)
            },
        }
    }

    /// Write user data from `addr` to file in total `count` bytes.
    /// Return the acutal conut of bytes written.
    pub fn fwrite(&self, addr: Address, count: usize) -> Result<usize, ()> {
        if !self.writable {
            return Err(())
        }

        match self.inner {
            FileInner::Pipe => todo!("pipe write"),
            FileInner::Regular(ref file) => {
                let batch = ((MAXOPBLOCKS-4)/2*BSIZE) as u32;
                let count_u32 = count as u32;
                let mut addr = addr;
                for i in (0..count_u32).step_by(batch as usize) {
                    let write_count = min(batch, count_u32 - i);
                    LOG.begin_op();
                    let mut idata = file.inode.as_ref().unwrap().lock();
                    let ret = idata.try_iwrite(addr, file.offset, write_count);
                    drop(idata);
                    LOG.end_op();

                    match ret {
                        Ok(actual_count) => {
                            // file.offset += actual_count; TODO
                            if actual_count != write_count {
                                return Ok((i+actual_count) as usize)
                            }
                        },
                        Err(()) => return Err(()),
                    }
                    addr = addr.offset(write_count as usize);
                }
                Ok(count)
            },
            FileInner::Device(ref dev) => {
                let dev_write = DEVICES[dev.major as usize].as_ref().ok_or(())?.write;
                dev_write(addr, count)
            },
        }
    }

    /// Copy the file status to user memory.
    pub fn fstat(&self, stat: &mut FileStat) -> Result<(), ()> {
        let inode: &Inode;
        match self.inner {
            FileInner::Pipe => return Err(()),
            FileInner::Regular(ref file) => inode = file.inode.as_ref().unwrap(),
            FileInner::Device(ref dev) => inode = dev.inode.as_ref().unwrap(),
        }
        let idata = inode.lock();
        idata.istat(stat);
        Ok(())
    }
}

impl Drop for File {
    /// Close the file.
    fn drop(&mut self) {
        match self.inner {
            FileInner::Pipe => todo!(),
            FileInner::Regular(ref mut file) => {
                LOG.begin_op();
                drop(file.inode.take());
                LOG.end_op();
            },
            FileInner::Device(ref mut dev) => {
                LOG.begin_op();
                drop(dev.inode.take());
                LOG.end_op();
            },
        }
    }
}

#[derive(Debug)]
enum FileInner {
    Pipe,
    Regular(FileRegular),
    Device(FileDevice),
}

#[derive(Debug)]
struct FileRegular {
    offset: u32,
    inode: Option<Inode>,
}

#[derive(Debug)]
struct FileDevice {
    major: u16,
    inode: Option<Inode>,
}
