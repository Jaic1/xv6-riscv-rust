use core::mem;

use crate::consts::{PGSHIFT, SV39FLAGLEN};
use crate::mm::{VirtAddr, PhysAddr};
use crate::mm::{kalloc, kfree, PageAligned};
use crate::mm::Box;

bitflags! {
    pub struct PteFlag: usize {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

/// PTE struct used in PageTable
///
/// It is not suitable to implement this with enum types,
/// because the lower 10-bits are used for flags.
/// So we need to do extra non-trivial conversion between its data and Box<PageTable>.
#[repr(C)]
struct PageTableEntry {
    data: usize,
}

impl PageTableEntry {
    #[inline]
    fn is_valid(&self) -> bool {
        (self.data & (PteFlag::V.bits()))  > 0
    }

    #[inline]
    fn as_page_table(&self) -> *mut PageTable {
        ((self.data >> SV39FLAGLEN) << PGSHIFT) as *mut PageTable
    }

    #[inline]
    fn write_zero(&mut self) {
        self.data = 0;
    }

    #[inline]
    fn write(&mut self, page_table: usize) {
        self.data = ((page_table >> PGSHIFT) << SV39FLAGLEN)
            | (PteFlag::V.bits());
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    data: [PageTableEntry; 512]
}

impl PageAligned for PageTable {}

impl PageTable {
    /// clear all bits to zero, typically called after Box::new()
    pub fn clear(&mut self) {
        for pte in self.data.iter_mut() {
            pte.write_zero();
        }
    }

    /// Create PTEs for virtual addresses starting at va that refer to
    /// physical addresses starting at pa. va and size might not
    /// be page-aligned. Returns Ok(()) on success, Err(_) if walk() couldn't
    /// allocate a needed page-table page.
    pub fn map_pages(
        &mut self,
        va: VirtAddr,
        size: usize,
        pa: PhysAddr,
        perm: PteFlag,
    ) -> Result<(), &'static str> {
        // TODO - may modify VirtAddr and PhyAddr first
        Ok(())
    }

    fn walk(&mut self, va: VirtAddr, alloc: bool) -> Option<&mut PageTableEntry> {
        let mut page_table = self as *mut PageTable;
        for level in (1..=2).rev() {
            let pte = unsafe {
                &mut (*page_table).data[va.page_num(level)]
            };

            if pte.is_valid() {
                page_table = pte.as_page_table() ;
            } else {
                if !alloc {
                    return None;
                }
                match Box::<PageTable>::new() {
                    Some(mut new_page_table) => {
                        new_page_table.clear();
                        page_table = new_page_table.into_raw();
                        pte.write(page_table as usize);
                    },
                    None => return None,
                }
            }
        }
        unsafe {
            Some(&mut (*page_table).data[va.page_num(0)])
        }
    }
}

