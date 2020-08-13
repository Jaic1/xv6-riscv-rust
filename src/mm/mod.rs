//! memory management module

pub use addr::{Addr, PhysAddr, VirtAddr};
pub use boxed::{Box, PageAligned};
pub use kalloc::{kalloc, kfree, kinit};
pub use kvm::{kvm_init, kvm_init_hart, kvm_map, kvm_pa};
pub use pagetable::{PageTable, PteFlag};

pub mod addr;
pub mod boxed;
pub mod kalloc;
pub mod kvm;
pub mod pagetable;
