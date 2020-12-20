use core::convert::TryFrom;
use core::option::Option;
use core::ptr::{self, NonNull};

use crate::consts::{PGSIZE, PHYSTOP};
use crate::mm::{Addr, PhysAddr};
use crate::spinlock::SpinLock;

#[repr(C)]
struct Frame {
    next: Option<NonNull<Frame>>,
}

unsafe impl Sync for Frame {}

impl Frame {
    unsafe fn new(ptr: *mut u8) -> NonNull<Frame> {
        let frame_ptr = ptr as *mut Frame;
        ptr::write(frame_ptr, Frame { next: None });
        NonNull::new(frame_ptr).unwrap()
    }

    fn set(&mut self, value: Option<NonNull<Frame>>) {
        self.next = value;
    }

    fn take_next(&mut self) -> Option<NonNull<Frame>> {
        self.next.take()
    }
}

type FrameList = Frame;

static KMEM: SpinLock<FrameList> = SpinLock::new(FrameList { next: None }, "kmem");

// must only be called once by a single hart
pub unsafe fn kinit() {
    extern "C" {
        fn end();
    }
    let end = end as usize;
    println!("kinit: end={:#x}", end);
    free_range(
        PhysAddr::try_from((end + PGSIZE - 1) & !(PGSIZE - 1)).unwrap(),
        PhysAddr::try_from(PHYSTOP).unwrap(),
    );
    println!("kinit: done");
}

// only used in kinit()
unsafe fn free_range(mut start: PhysAddr, end: PhysAddr) {
    start.pg_round_up();
    while start != end {
        kfree(start.as_usize() as *mut u8);
        start.add_page();
    }
}

/// Free the page of physical memory pointed at by the non-null pointer,
/// which normally should have been returned by a
/// call to kalloc().  (The exception is when
/// initializing the allocator; see kinit above.)
pub unsafe fn kfree(ptr: *mut u8) {
    let mut frame: NonNull<Frame> = Frame::new(ptr);
    let mut kmem = KMEM.lock();
    frame.as_mut().set(kmem.take_next());
    kmem.set(Some(frame));
    drop(kmem);
}

pub unsafe fn kalloc() -> Option<*mut u8> {
    let mut kmem = KMEM.lock();
    let first_frame = kmem.take_next();
    if let Some(mut first_frame_ptr) = first_frame {
        kmem.set(first_frame_ptr.as_mut().take_next());
    }
    drop(kmem);

    match first_frame {
        Some(first_frame_ptr) => Some(first_frame_ptr.as_ptr() as *mut u8),
        None => None,
    }
}

#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;
    use crate::consts;
    use crate::proc::cpu_id;
    use crate::mm::pagetable::PageTable;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn alloc_simo() {
        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != NSMP as u8 {}

        let id = unsafe { cpu_id() };

        for _ in 0..10 {
            let page_table = PageTable::new();
            println!("hart {} alloc page table at {:#x}", id, page_table.addr());
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
