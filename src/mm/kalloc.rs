use crate::consts::memlayout::PHYSTOP;
use crate::mm::addr::PhysAddr;
use crate::mm::PGSIZE;
use crate::spinlock::SpinLock;
use core::option::Option;
use core::ptr::{self, NonNull};

#[repr(C)]
struct Frame {
    next: Option<NonNull<Frame>>,
}

unsafe impl Send for Frame {}

impl Frame {
    /// Convert from PhysAddr to a new Frame
    /// note that it will consume the PhysAddr
    unsafe fn from(pa: PhysAddr) -> &'static mut Frame {
        let frame_ptr = pa.into_mut_ptr::<Frame>();
        ptr::write(frame_ptr, Frame { next: None });
        &mut *frame_ptr
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
// static mut KMEM: FrameList = FrameList { next: None };

pub fn kinit() {
    extern "C" {
        fn end();
    }
    println!("kinit: end={:#x}", end as usize);
    free_range(end as usize, PHYSTOP);
    println!("kinit: done");
}

fn free_range(start: usize, end: usize) {
    let start = super::pg_round_up(start);
    for pa in (start..end).step_by(PGSIZE) {
        kfree(PhysAddr::new(pa).unwrap());
    }
}

/// Free the page of physical memory pointed at by pa,
/// which normally should have been returned by a
/// call to kalloc().  (The exception is when
/// initializing the allocator; see kinit above.)
pub fn kfree(pa: PhysAddr) {
    let frame = unsafe { Frame::from(pa) };

    let mut kmem = KMEM.lock();
    frame.set(kmem.take_next());
    kmem.set(Some(NonNull::from(frame)));
    drop(kmem);
}

pub fn kalloc() -> Option<PhysAddr> {
    let mut kmem = KMEM.lock();
    let first_frame = kmem.take_next();
    if let Some(mut first_frame_ptr) = first_frame {
        unsafe {
            kmem.set(first_frame_ptr.as_mut().take_next());
        }
    }
    drop(kmem);

    match first_frame {
        Some(first_frame_ptr) => {
            Some(PhysAddr::new(first_frame_ptr.as_ptr() as *const _ as usize).unwrap())
        }
        None => None,
    }
}

#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;
    use crate::consts::param;
    use crate::proc::cpu_id;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn alloc_simo() {
        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != param::NSMP as u8 {}

        for _ in 0..10 {
            let pa = kalloc().expect("Not enough frame");
            kfree(pa);
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
