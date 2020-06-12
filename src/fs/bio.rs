//! buffer cache layer

use crate::spinlock::SpinLock;
use crate::driver::virtio;

use super::NBUF;
use super::Buf;

static mut BCACHE: Bcache = Bcache::new();

struct Bcache {
    lock: SpinLock<()>,
    bufs: [Buf; NBUF],
    head: Buf,
}

impl Bcache {
    const fn new() -> Self {
        Self {
            lock: SpinLock::new((), "bcache"),
            bufs: [Buf::new(); NBUF],
            head: Buf::new(),
        }
    }
}

pub unsafe fn binit() {
    // Create linked list of buffers
    // BCACHE.head.prev = Some(&mut BCACHE.head as *mut Buf);
    // BCACHE.head.next = Some(&mut BCACHE.head as *mut Buf);
    // for b in BCACHE.bufs.iter_mut() {
    //     b.next = BCACHE.head.next.clone();
    //     b.prev = Some(&mut BCACHE.head as *mut Buf);
    //     let ori_next = BCACHE.head.next.replace(b as *mut Buf).unwrap();
    //     (*ori_next).prev = Some(b as *mut Buf);
    // }
}

/// Look through buffer cache for block on device dev.
/// If not found, allocate a buffer.
/// In either case, return locked buffer.
/// LTODO - just loop the bufs to find empty(refcnt=0) buffer
unsafe fn bget(dev: u32, blockno: u32) -> &'static Buf {
    let bcache = BCACHE.lock.lock();

    // find exist buffer first
    for b in BCACHE.bufs.iter_mut() {
        if b.refcnt > 0 && b.dev == dev && b.blockno == blockno {
            b.refcnt += 1;
            drop(bcache);
            return b
        }
    }

    // find empty buffer then
    for b in BCACHE.bufs.iter_mut() {
        if b.refcnt == 0 {
            b.refcnt += 1;
            b.dev = dev;
            b.blockno = blockno;
            b.valid.set(false);
            drop(bcache);
            return b
        }
    }

    panic!("bget: could not find empty buffer")
}


pub fn bread(dev: u32, blockno: u32) -> &'static Buf {
    let b = unsafe {bget(dev, blockno)};
    if !b.valid.get() {
        unsafe {virtio::disk_rw(b, false)};
        b.valid.set(true);
    }
    b
}

/// Release a ~locked~ buffer
/// ~Move to the head of the MRU list~
pub fn brelse(dev: u32, blockno: u32) {
    let _lock = unsafe {BCACHE.lock.lock()};

    // loop through the bcache bufs to
    // find current buf to get its mut reference
    let mut found: bool = false;
    for b in unsafe {BCACHE.bufs.iter_mut()} {
        if b.dev == dev && b.blockno == blockno {
            found = true;
            b.refcnt -= 1;
            break;
        }
    }
    
    if !found {
        panic!("brelse: not found buf(dev={}, blockno={})", dev, blockno);
    }
    drop(_lock);
}
