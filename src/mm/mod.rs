use alloc::boxed::Box;
use core::{alloc::AllocError, ptr};

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

/// Used to alloc pages-sized and page-aligned memory.
/// The impl typically using Box::new() and then Box::into_raw(). 
pub trait RawPage: Sized {
    /// Allocate an zeroed physical page.
    /// Return the raw pointer at the starting address of this page.
    unsafe fn new_zeroed() -> *mut u8 {
        let boxed_page = Box::<Self>::new_zeroed().assume_init();
        Box::into_raw(boxed_page) as *mut u8
    }

    /// Try to allocate an zeroed physical page.
    /// If succeed, return the raw pointer at the starting address of this page.
    /// otherwise, return an [`AllocError`].
    unsafe fn try_new_zeroed() -> Result<*mut u8, AllocError> {
        let boxed_page = Box::<Self>::try_new_zeroed()?.assume_init();
        Ok(Box::into_raw(boxed_page) as *mut u8)
    }

    /// Try to allocate an uninitialized physical page.
    /// If succeed, return the raw pointer at the starting address of this page.
    /// otherwise, return an [`AllocError`].
    unsafe fn try_new_uninit() -> Result<*mut u8, AllocError> {
        let boxed_page = Box::<Self>::try_new_uninit()?.assume_init();
        Ok(Box::into_raw(boxed_page) as *mut u8)
    }

    /// Reconstructs the box from the previously handed-out raw pointer.
    /// And then drop the box.
    unsafe fn from_raw_and_drop(raw: *mut u8) {
        drop(Box::from_raw(raw as *mut Self));
    }
}

/// Used to alloc single-page-sized and page-aligned memory.
#[repr(C, align(4096))]
pub struct RawSinglePage {
    data: [u8; PGSIZE]
}

impl RawPage for RawSinglePage {}

/// Used to alloc double-page-sized and page-aligned memory.
/// Similar to [`RawSinglePage`].
#[repr(C, align(4096))]
pub struct RawDoublePage {
    data: [u8; PGSIZE*2]
}

impl RawPage for RawDoublePage {}

/// Used to alloc quadruple-page-sized and page-aligned memory.
/// Similar to [`RawSinglePage`].
#[repr(C, align(4096))]
pub struct RawQuadPage {
    data: [u8; PGSIZE*4]
}

impl RawPage for RawQuadPage {}

#[derive(Clone, Copy, Debug)]
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

#[inline]
pub fn pg_round_up(address: usize) -> usize {
    (address + (PGSIZE - 1)) & !(PGSIZE - 1)
}

#[inline]
pub fn pg_round_down(address: usize) -> usize {
    address & !(PGSIZE - 1)
}
