use core::convert::TryFrom;
use core::ptr;

use crate::consts::{PGSIZE, PGSHIFT, SATP_SV39, SV39FLAGLEN, USERTEXT};

use super::Box;
use super::kalloc;
use super::PageAligned;
use super::{Addr, PhysAddr, VirtAddr};

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
pub struct PageTableEntry {
    data: usize,
}

impl PageTableEntry {
    #[inline]
    pub fn is_valid(&self) -> bool {
        (self.data & (PteFlag::V.bits())) > 0
    }

    #[inline]
    fn is_user(&self) -> bool {
        (self.data & (PteFlag::U.bits())) > 0
    }

    #[inline]
    fn as_page_table(&self) -> *mut PageTable {
        ((self.data >> SV39FLAGLEN) << PGSHIFT) as *mut PageTable
    }

    #[inline]
    pub fn as_phys_addr(&self) -> PhysAddr {
        PhysAddr::try_from((self.data >> SV39FLAGLEN) << PGSHIFT).unwrap()
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
    pub fn as_satp(&self) -> usize {
        SATP_SV39 | ((self.data.as_ptr() as usize) >> PGSHIFT)
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
            match self.walk_alloc(va) {
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
    fn walk_alloc(&mut self, va: VirtAddr) -> Option<&mut PageTableEntry> {
        let mut page_table = self as *mut PageTable;
        for level in (1..=2).rev() {
            let pte = unsafe { &mut (*page_table).data[va.page_num(level)] };

            if pte.is_valid() {
                page_table = pte.as_page_table();
            } else {
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

    pub fn walk(&self, va: VirtAddr) -> Option<&PageTableEntry> {
        let mut page_table = self as *const PageTable;
        for level in (1..=2).rev() {
            let pte = unsafe { &(*page_table).data[va.page_num(level)] };

            if pte.is_valid() {
                page_table = pte.as_page_table();
            } else {
                return None
            }
        }
        unsafe { Some(&(*page_table).data[va.page_num(0)]) }
    }

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

    /// Load the initcode and map it into the pagetable
    /// Only used for the very first process
    pub fn uvm_init(&mut self, code: &[u8]) {
        if code.len() >= PGSIZE {
            panic!("pagetable's uvm_init: initcode more than a page");
        }
        match unsafe{kalloc()} {
            Some(mem) => {
                unsafe {ptr::write_bytes(mem, 0, PGSIZE);}
                self.map_pages(VirtAddr::from(USERTEXT), PGSIZE,
                    PhysAddr::try_from(mem as usize).unwrap(),
                    PteFlag::R | PteFlag::W | PteFlag::X | PteFlag::U)
                    .expect("pagetable's uvm_init map_pages: ");
                unsafe {ptr::copy_nonoverlapping(code.as_ptr(),
                    mem, code.len());}
            }
            None => {
                panic!("pagetable's uvm_init: not enough memory");
            }
        }
    }

    /// Return the mapped physical address(page aligned)
    /// va need not be page aligned
    fn walk_addr(&self, va: VirtAddr)
        -> Result<PhysAddr, &'static str>
    {
        match self.walk(va) {
            Some(pte) => {
                if !pte.is_valid() {
                    Err("pte not valid")
                } else if !pte.is_user() {
                    Err("pte not mapped for user")
                } else {
                    Ok(pte.as_phys_addr())
                }
            }
            None => {
                Err("va not mapped")
            }
        }
    }

    /// Copy null-terminated string from virtual address starting at srcva,
    /// to a kernel u8 slice.
    pub fn copy_in_str(&self, srcva: usize, dst: &mut [u8])
        -> Result<(), &'static str>
    {
        let mut i: usize = 0;
        let mut va = VirtAddr::try_from(srcva).unwrap();

        // iterate through the raw content page by page
        while i < dst.len() {
            let mut base = va;
            base.pg_round_down();
            let distance = (va - base).as_usize();
            let mut pa_ptr = unsafe {
                self.walk_addr(base)?
                    .as_ptr()
                    .offset(distance as isize)
            };
            let mut va_ptr = va.as_ptr();
            base.add_page();
            let va_end = base.as_ptr();
            va = base;

            // iterate througn each u8 in a page
            let enough_space = (dst.len() - i) >= (PGSIZE - distance);
            while !ptr::eq(va_ptr, va_end) {
                if !enough_space && i >= dst.len() {
                    return Err("copy_in_str: dst not enough space")
                }

                unsafe {
                    dst[i] = *pa_ptr;
                    if dst[i] == 0 {
                        return Ok(())
                    }
                    i += 1;
                    pa_ptr = pa_ptr.add(1);
                    va_ptr = va_ptr.add(1);
                }
            }
        }

        Err("copy_in_str: dst not enough space")
    }
}
