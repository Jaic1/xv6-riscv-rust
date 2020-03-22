use crate::consts::memlayout::PHYSTOP;
use core::option::Option;
use core::ptr::{self, NonNull};
use linked_list_allocator::LockedHeap;

#[repr(C)]
struct Frame {
    next: Option<NonNull<Frame>>,
}

unsafe impl Send for Frame {}

impl Frame {
    /// Convert from raw addr to a new Frame
    /// only used when then kernel boots and call kinit
    unsafe fn new(pa: usize) -> NonNull<Frame> {
        let frame_ptr = pa as *mut Frame;
        ptr::write(frame_ptr, Frame { next: None });
        NonNull::new(frame_ptr).unwrap()
    }

    /// mut ref of unfreed_data
    /// transmute to
    /// mut ref of Frame
    ///
    /// used in kfree
    fn from<T>(unfreed_data: &mut T) -> &mut Frame {
        unsafe { &mut *(unfreed_data as *mut T as *mut Frame) }
    }

    fn into<T>(&mut self) -> &mut T {
        unsafe { &mut *(self as *mut Self as *mut T) }
    }

    fn set(&mut self, value: Option<NonNull<Frame>>) {
        self.next = value;
    }

    fn take_next(&mut self) -> Option<NonNull<Frame>> {
        self.next.take()
    }
}

type FrameList = Frame;

// static KMEM: SpinLock<FrameList> = SpinLock::new(FrameList { next: None }, "kmem");

// must only be called once by a single hart
pub unsafe fn kinit() {
    extern "C" {
        fn end();
    }
    println!("kinit: end={:#x}", end as usize);
    // free_range(end as usize, PHYSTOP);
    ALLOCATOR
        .lock()
        .init(super::pg_round_up(end as usize), PHYSTOP);
    println!("kinit: done");
}

// for linked list allocator
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
// end

// unsafe fn free_range(start: usize, end: usize) {
//     let start = super::pg_round_up(start);
//     for pa in (start..end).step_by(PGSIZE) {
//         kfree(Frame::new(pa).as_mut());
//     }
// }

/// Free the page of physical memory pointed at by pa,
/// which normally should have been returned by a
/// call to kalloc().  (The exception is when
/// initializing the allocator; see kinit above.)
///
/// TODO - T must have 4KB size
// pub fn kfree<T>(unfreed_data: &mut T) {
//     let frame = Frame::from(unfreed_data);
//
//     let mut kmem = KMEM.lock();
//     frame.set(kmem.take_next());
//     kmem.set(Some(NonNull::from(frame)));
//     drop(kmem);
// }

// pub fn kalloc<T>() -> Option<&'static mut T> {
//     let mut kmem = KMEM.lock();
//     let first_frame = kmem.take_next();
//     if let Some(mut first_frame_ptr) = first_frame {
//         unsafe {
//             kmem.set(first_frame_ptr.as_mut().take_next());
//         }
//     }
//     drop(kmem);
//
//     match first_frame {
//         Some(first_frame_ptr) => {
//             // TODO
//             Some(first_frame_ptr.cast::<T>().as_mut())
//         }
//         None => None,
//     }
// }
#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;
    use crate::consts::param;
    use crate::mm::pagetable::{PageTable, PageTableEntry};
    use crate::proc::cpu_id;
    use alloc::boxed::Box;
    use core::sync::atomic::{AtomicU8, Ordering};

    pub fn alloc_simo() {
        // use NSMP to synchronize testing pr's spinlock
        static NSMP: AtomicU8 = AtomicU8::new(0);
        NSMP.fetch_add(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != param::NSMP as u8 {}

        for _ in 0..10 {
            let pt = unsafe { Box::<PageTable>::new_zeroed().assume_init() };
            let ptr = Box::into_raw(pt);
            println!("new pagetable start at {:p}", ptr);
            unsafe {
                Box::from_raw(ptr);
            }
        }

        NSMP.fetch_sub(1, Ordering::Relaxed);
        while NSMP.load(Ordering::Relaxed) != 0 {}
    }
}
