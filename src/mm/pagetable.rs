#[repr(C)]
pub struct PageTableEntry {
    pub data: usize,
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub data: [PageTableEntry; 512],
}

// impl PageTable {
//     fn new() -> &'static mut PageTable {
//         kalloc::<PageTable>().expect("pagetable new failed")
//     }
// }

// impl Drop for PageTable {
//     fn drop(&mut self) {
//         kfree()
//     }
// }
