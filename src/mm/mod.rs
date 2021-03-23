use alloc::boxed::Box;

use crate::consts::PGSIZE;

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
