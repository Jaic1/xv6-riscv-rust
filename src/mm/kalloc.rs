use crate::consts::memlayout::PHYSTOP;
use crate::mm::PGSIZE;
use crate::spinlock::SpinLock;
use core::ops::{Deref, DerefMut};
use core::option::Option;
use core::ptr::{self, NonNull};

pub trait PageAligned {}

#[repr(C)]
struct Frame {
    next: Option<NonNull<Frame>>,
}

unsafe impl Send for Frame {}

impl PageAligned for Frame {}

impl Frame {
    /// Convert from raw addr to a new Frame
    /// only used when then kernel boots and call kinit
    unsafe fn new(pa: usize) -> NonNull<Frame> {
        let frame_ptr = pa as *mut Frame;
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

#[repr(C)]
struct FrameWrapper {
    ptr: NonNull<Frame>,
}

impl<T: PageAligned> From<NonNull<T>> for FrameWrapper {
    fn from(ptr: NonNull<T>) -> Self {
        FrameWrapper { ptr: ptr.cast() }
    }
}

impl FrameWrapper {
    pub fn into_raw_non_null(self) -> NonNull<Frame> {
        self.ptr
    }
}

impl Deref for FrameWrapper {
    type Target = Frame;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl DerefMut for FrameWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

type FrameList = Frame;

static KMEM: SpinLock<FrameList> = SpinLock::new(FrameList { next: None }, "kmem");

// must only be called once by a single hart
pub unsafe fn kinit() {
    extern "C" {
        fn end();
    }
    println!("kinit: end={:#x}", end as usize);
    free_range(end as usize, PHYSTOP);
    println!("kinit: done");
}

unsafe fn free_range(start: usize, end: usize) {
    let start = super::pg_round_up(start);
    for pa in (start..end).step_by(PGSIZE) {
        kfree(Frame::new(pa));
    }
}

/// Free the page of physical memory pointed at by the non-null pointer,
/// which normally should have been returned by a
/// call to kalloc().  (The exception is when
/// initializing the allocator; see kinit above.)
pub fn kfree<T: PageAligned>(unfreed_data: NonNull<T>) {
    let mut frame = FrameWrapper::from(unfreed_data);
    let mut kmem = KMEM.lock();
    frame.set(kmem.take_next());
    kmem.set(Some(frame.into_raw_non_null()));
    drop(kmem);
}

pub fn kalloc<T: PageAligned>() -> Option<NonNull<T>> {
    let mut kmem = KMEM.lock();
    let first_frame = kmem.take_next();
    if let Some(mut first_frame_ptr) = first_frame {
        unsafe {
            kmem.set(first_frame_ptr.as_mut().take_next());
        }
    }
    drop(kmem);

    match first_frame {
        Some(first_frame_ptr) => Some(first_frame_ptr.cast()),
        None => None,
    }
}

#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;
    use crate::consts::param;
    use crate::mm::pagetable::PageTable;
    use crate::proc::cpu_id;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn alloc_simo() {
        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != param::NSMP as u8 {}

        let id = unsafe { cpu_id() };

        for _ in 0..10 {
            let page_table = kalloc::<PageTable>();
            if let Some(page_table_ptr) = page_table {
                println!(
                    "hart {} alloc page table at {:#x}",
                    id,
                    page_table_ptr.as_ptr() as usize
                );
            }
            kfree(page_table.expect("alloc_simo fails"));
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
