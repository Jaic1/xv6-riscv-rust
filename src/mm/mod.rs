use alloc::boxed::Box;
use core::ptr;

use crate::consts::PGSIZE;
use crate::process::CPU_MANAGER;

pub use addr::{Addr, PhysAddr, VirtAddr};
pub use kvm::{kvm_init, kvm_init_hart, kvm_map, kvm_pa};
pub use pagetable::{PageTable, PteFlag};
pub use kalloc::{KernelHeap, KERNEL_HEAP};

mod addr;
pub mod kalloc;
mod kvm;
mod pagetable;
mod list;

/// Used to alloc page-sized memory.
/// Typically called with Box::new() and then Box::into_raw(). 
#[repr(C, align(4096))]
pub struct RawPage {
    data: [u8; PGSIZE]
}

impl RawPage {
    /// Allocate an zeroed physical page.
    /// Return the raw address of this page.
    pub unsafe fn new_zeroed() -> usize {
        let boxed_page = Box::<Self>::new_zeroed().assume_init();
        Box::into_raw(boxed_page) as usize
    }
}

#[derive(Clone, Copy)]
pub enum Address {
    Virtual(usize),
    Kernel(*const u8),
    KernelMut(*mut u8),
}

impl Address {
    /// Calculates the offset from this Virtual/Kernel Address.
    /// The passed-in count should be smaller than isize::MAX.
    pub fn offset(self, count: usize) -> Self {
        debug_assert!(count < (isize::MAX) as usize);
        match self {
            Self::Virtual(p) => Self::Virtual(p + count),
            Self::Kernel(p) => Self::Kernel(unsafe { p.offset(count as isize) }),
            Self::KernelMut(p) => Self::KernelMut(unsafe { p.offset(count as isize) }),
        }
    }

    /// Copy content from src to this Virtual/Kernel address.
    /// Copy `count` bytes in total.
    pub fn copy_out(self, src: *const u8, count: usize) -> Result<(), ()> {
        match self {
            Self::Virtual(dst) => {
                let p = unsafe { CPU_MANAGER.my_proc() };
                p.data.get_mut().copy_out(src, dst, count)
            },
            Self::Kernel(dst) => {
                panic!("cannot copy to a const pointer {:p}", dst)
            },
            Self::KernelMut(dst) => {
                unsafe { ptr::copy(src, dst, count); }
                Ok(())
            },
        }
    }

    /// Copy content from this Virtual/Kernel address to dst.
    /// Copy `count` bytes in total.
    pub fn copy_in(self, dst: *mut u8, count: usize) -> Result<(), ()> {
        match self {
            Self::Virtual(src) => {
                let p = unsafe { CPU_MANAGER.my_proc() };
                p.data.get_mut().copy_in(src, dst, count)
            },
            Self::Kernel(src) => {
                unsafe { ptr::copy(src, dst, count); }
                Ok(())
            },
            Self::KernelMut(src) => {
                debug_assert!(false);
                unsafe { ptr::copy(src, dst, count); }
                Ok(())
            },
        }
    }
}
