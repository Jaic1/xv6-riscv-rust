use core::convert::TryFrom;

use crate::consts::{PGSHIFT, SATP_SV39, SV39FLAGLEN};
use crate::mm::Box;
use crate::mm::PageAligned;
use crate::mm::{Addr, PhysAddr, VirtAddr};

bitflags! {
    pub struct PteFlag: usize {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
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
        (self.data & (PteFlag::V.bits())) > 0
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
    fn write(&mut self, pa: PhysAddr) {
        self.data = ((pa.as_usize() >> PGSHIFT) << SV39FLAGLEN) | (PteFlag::V.bits());
    }

    #[inline]
    fn write_perm(&mut self, pa: PhysAddr, perm: PteFlag) {
        self.data = ((pa.as_usize() >> PGSHIFT) << SV39FLAGLEN) | (perm | PteFlag::V).bits()
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    data: [PageTableEntry; 512],
}

impl PageAligned for PageTable {}

impl PageTable {
    pub const fn empty() -> Self {
        Self {
            data: [PageTableEntry { data: 0 }; 512],
        }
    }

    /// clear all bits to zero, typically called after Box::new()
    pub fn clear(&mut self) {
        for pte in self.data.iter_mut() {
            pte.write_zero();
        }
    }

    /// Convert the page table to be the usize
    /// that can be written in satp register
    pub unsafe fn as_satp(&self) -> usize {
        SATP_SV39 | ((&self.data as *const _ as usize) >> PGSHIFT)
    }

    /// Create PTEs for virtual addresses starting at va that refer to
    /// physical addresses starting at pa. va and size might not
    /// be page-aligned. Returns Ok(()) on success, Err(_) if walk() couldn't
    /// allocate a needed page-table page.
    pub fn map_pages(
        &mut self,
        mut va: VirtAddr,
        size: usize,
        mut pa: PhysAddr,
        perm: PteFlag,
    ) -> Result<(), &'static str> {
        let mut last = VirtAddr::try_from(va.as_usize() + size)?;
        va.pg_round_down();
        last.pg_round_up();

        while va != last {
            match self.walk(va, true) {
                Some(pte) => {
                    if pte.is_valid() {
                        println!(
                            "va: {:#x}, pa: {:#x}, pte: {:#x}",
                            va.as_usize(),
                            pa.as_usize(),
                            pte.data
                        );
                        panic!("remap");
                    }
                    pte.write_perm(pa, perm);
                    va.add_page();
                    pa.add_page();
                }
                None => {
                    return Err("PageTable.map_pages: \
                    not enough memory for new page table")
                }
            }
        }

        Ok(())
    }

    /// Return the bottom level of PTE that corresponds to the given va.
    /// i.e. this PTE contains the pa that is mapped for the given va.
    ///
    /// if alloc is true then allocate new page table necessarily
    /// but doesn't change anything.(lazy allocation)
    fn walk(&mut self, va: VirtAddr, alloc: bool) -> Option<&mut PageTableEntry> {
        let mut page_table = self as *mut PageTable;
        for level in (1..=2).rev() {
            let pte = unsafe { &mut (*page_table).data[va.page_num(level)] };

            if pte.is_valid() {
                page_table = pte.as_page_table();
            } else {
                if !alloc {
                    return None;
                }
                match Box::<PageTable>::new() {
                    Some(mut new_page_table) => {
                        new_page_table.clear();
                        page_table = new_page_table.into_raw();
                        pte.write(PhysAddr::try_from(page_table as usize).unwrap());
                    }
                    None => return None,
                }
            }
        }
        unsafe { Some(&mut (*page_table).data[va.page_num(0)]) }
    }
}
