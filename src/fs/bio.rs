//! buffer cache layer

use array_const_fn_init::array_const_fn_init;

use core::ptr;

use crate::spinlock::SpinLock;
use crate::driver::virtio;
use super::{NBUF, BSIZE};

static BCACHE: SpinLock<Bcache> = SpinLock::new(Bcache::new(), "bcache");

struct Bcache {
    bufs: [Buf; NBUF],
    head: Buf,
}

// Temp solve the problem that Buf is not Sync by default
unsafe impl Sync for Bcache {}

impl Bcache {
    const fn new() -> Self {
        Self {
            bufs: array_const_fn_init![buf_new; 30],    // 30 is NBUF
            head: Buf::new(),
        }
    }
}

/// Init the bcache.
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
    panic!("binit undone");
}

/// Look through buffer cache for block on device dev.
/// If not found, allocate a buffer.
/// In either case, return locked buffer.
/// TODO - just loop the bufs to find empty(refcnt=0) buffer
unsafe fn bget(_dev: u32, _blockno: u32) -> &'static mut Buf {
    // let mut guard = BCACHE.lock();

    // // find exist buffer first
    // for b in guard.bufs.iter_mut() {
    //     if b.refcnt > 0 && b.dev == dev && b.blockno == blockno {
    //         b.refcnt += 1;
    //         drop(guard);
    //         return b
    //     }
    // }

    // // find empty buffer then
    // for b in guard.bufs.iter_mut() {
    //     if b.refcnt == 0 {
    //         b.refcnt += 1;
    //         b.dev = dev;
    //         b.blockno = blockno;
    //         b.valid = false;
    //         drop(guard);
    //         return b
    //     }
    // }

    // panic!("bget: could not find empty buffer")
    panic!("bget undone");
}


pub fn bread(dev: u32, blockno: u32) -> &'static mut Buf {
    let b = unsafe {bget(dev, blockno)};
    if !b.valid {
        unsafe {virtio::disk_rw(b, false)};
        b.valid = true;
    }
    b
}

/// Release a ~locked~ buffer
/// ~Move to the head of the MRU list~
pub fn brelse(dev: u32, blockno: u32) {
    let mut guard = BCACHE.lock();

    // loop through the bcache bufs to
    // find current buf to get its mut reference
    let mut found: bool = false;
    for b in guard.bufs.iter_mut() {
        if b.dev == dev && b.blockno == blockno {
            found = true;
            b.refcnt -= 1;
            break;
        }
    }
    
    if !found {
        panic!("brelse: not found buf(dev={}, blockno={})", dev, blockno);
    }
    drop(guard);
}

/// TODO - may consider rc?
pub struct Buf {
    pub valid: bool,
    pub disk: bool,
    pub dev: u32,
    pub blockno: u32,
    pub refcnt: usize,
    pub prev: *const Buf,     // TODO - 12/19 use smart pointers?
    pub next: *const Buf,
    pub data: [u8; BSIZE],
}

const fn buf_new(_: usize) -> Buf {
    Buf::new()
}

impl Buf {
    const fn new() -> Self {
        Self {
            valid: false,
            disk: false,
            dev: 0,
            blockno: 0,
            refcnt: 0,
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
            data: [0; BSIZE],
        }
    }
}
