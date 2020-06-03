use super::{Box, PageTable};

/// Create a page table for a given process,
/// with no user pages, but with trampoline pages.
pub fn uvm_create() -> Box<PageTable> {
    match Box::<PageTable>::new() {
        Some(mut pagetable) => {
            pagetable.clear();
            pagetable
        },
        None => {
            panic!("uvm_create: out of memory");
        },
    }
}
