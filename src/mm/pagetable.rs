use crate::mm::addr::VirtAddr;
use crate::mm::kalloc::PageAligned;

#[repr(usize)]
pub enum PteFlag {
    V = 1 << 0,
    R = 1 << 1,
    W = 1 << 2,
    X = 1 << 3,
    U = 1 << 4,
}

/// PTE struct used in PageTable
///
/// It is not suitable to implement this with enum types,
/// because the lower 10-bits are used for flags.
/// So we need to do extra non-trivial conversion between its data and Box<PageTable>.
#[repr(transparent)]
struct PageTableEntry {
    data: usize,
}

impl PageTableEntry {
    #[inline]
    fn is_valid(&self) -> bool {
        self.data & (PteFlag::V as usize) > 0
    }

    #[inline]
    fn as_pa(&self) -> usize {
        (self.data >> 10) << 12
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    data: [PageTableEntry; 512],
}

impl PageAligned for PageTable {}

impl PageTable {
    /// Create PTEs for virtual addresses starting at va that refer to
    /// physical addresses starting at pa. va and size might not
    /// be page-aligned. Returns Ok(()) on success, Err(String) if walk() couldn't
    /// allocate a needed page-table page.
    pub fn map_pages(
        &mut self,
        va: usize,
        size: usize,
        pa: usize,
        perm: PteFlag,
    ) -> Result<(), &'static str> {
        Ok(())
    }

    /// Return the reference of the PTE in page table pagetable
    /// that corresponds to virtual address va. If alloc is true,
    /// create any required page-table pages.
    // fn walk_ref(&mut self, va: usize, alloc: bool) -> &PageTableEntry {
    //
    // }

    /// Same as walk_ref, but return a mutable reference
    fn walk_mut(&mut self, va: VirtAddr, alloc: bool) -> Option<&mut PageTableEntry> {
        let mut page_table = self as *mut PageTable;
        for level in (1..=2).rev() {
            // this &mut of data is safe,
            // because this mut ref is inside function with &mut self
            // *mut PageTableEntry may be better?
            let mut pte = unsafe { &mut (*page_table).data[va.page_num(level)] };
            if (pte.is_valid()) {
                page_table = pte.as_pa() as *mut PageTable;
            } else {
                // TODO - Box::new will not return error, i.e.,
                // TODO - we need a way to handle heap allocation error
            }
        }
        Some(&mut self.data[0])
    }
}
