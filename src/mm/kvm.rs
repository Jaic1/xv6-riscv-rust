use core::convert::{TryFrom, Into};
use core::mem;

use crate::consts::{
    CLINT, CLINT_MAP_SIZE, KERNBASE, PHYSTOP, PLIC, PLIC_MAP_SIZE, UART0, UART0_MAP_SIZE, VIRTIO0,
    VIRTIO0_MAP_SIZE, TRAMPOLINE, PGSIZE
};
use crate::register::satp;
use super::{Addr, PageTable, PhysAddr, PteFlag, VirtAddr, RawSinglePage, RawDoublePage, RawQuadPage};

static mut KERNEL_PAGE_TABLE: PageTable = PageTable::empty();

pub unsafe fn kvm_init_hart() {
    satp::write(KERNEL_PAGE_TABLE.as_satp());
    llvm_asm!("sfence.vma zero, zero"::::"volatile");
}

pub unsafe fn kvm_init() {
    // check if RawPages and PageTable have the same mem layout
    debug_assert_eq!(mem::size_of::<RawSinglePage>(), PGSIZE);
    debug_assert_eq!(mem::align_of::<RawSinglePage>(), PGSIZE);
    debug_assert_eq!(mem::size_of::<RawSinglePage>(), mem::size_of::<PageTable>());
    debug_assert_eq!(mem::align_of::<RawSinglePage>(), mem::align_of::<PageTable>());
    debug_assert_eq!(mem::size_of::<RawDoublePage>(), PGSIZE*2);
    debug_assert_eq!(mem::align_of::<RawDoublePage>(), PGSIZE);
    debug_assert_eq!(mem::size_of::<RawQuadPage>(), PGSIZE*4);
    debug_assert_eq!(mem::align_of::<RawQuadPage>(), PGSIZE);

    // uart registers
    kvm_map(
        VirtAddr::from(UART0),
        PhysAddr::from(UART0),
        UART0_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // virtio mmio disk interface
    kvm_map(
        VirtAddr::from(VIRTIO0),
        PhysAddr::from(VIRTIO0),
        VIRTIO0_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // CLINT
    kvm_map(
        VirtAddr::from(CLINT),
        PhysAddr::from(CLINT),
        CLINT_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // PLIC
    kvm_map(
        VirtAddr::from(PLIC),
        PhysAddr::from(PLIC),
        PLIC_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // etext exported out of kernel.ld
    // supposed to be page(0x1000) aligned
    extern "C" {
        fn etext();
    }
    let etext = etext as usize;

    // map kernel text executable and read-only.
    kvm_map(
        VirtAddr::from(KERNBASE),
        PhysAddr::from(KERNBASE),
        etext - Into::<usize>::into(KERNBASE),
        PteFlag::R | PteFlag::X,
    );

    // map kernel data and the physical RAM we'll make use of.
    kvm_map(
        VirtAddr::try_from(etext).unwrap(),
        PhysAddr::try_from(etext).unwrap(),
        usize::from(PHYSTOP) - etext,
        PteFlag::R | PteFlag::W,
    );

    // map the trampoline for trap entry/exit to
    // the highest virtual address in the kernel.
    extern "C" {
        fn trampoline();
    }
    kvm_map(
        VirtAddr::from(TRAMPOLINE),
        PhysAddr::try_from(trampoline as usize).unwrap(),
        PGSIZE,
        PteFlag::R | PteFlag::X
    );
}

pub unsafe fn kvm_map(va: VirtAddr, pa: PhysAddr, size: usize, perm: PteFlag) {
    #[cfg(feature = "verbose_init_info")]
    println!(
        "kvm_map: va={:#x}, pa={:#x}, size={:#x}",
        va.as_usize(),
        pa.as_usize(),
        size
    );

    if let Err(err) = KERNEL_PAGE_TABLE.map_pages(va, size, pa, perm) {
        panic!("kvm_map: {}", err);
    }
}

/// translate a kernel virtual address to
/// a physical address. only needed for
/// addresses on the stack.
/// va need not be page aligned.
pub unsafe fn kvm_pa(va: VirtAddr) -> u64 {
    let off: u64 = (va.as_usize() % PGSIZE) as u64;
    match KERNEL_PAGE_TABLE.walk(va) {
        Some(pte) => {
            if !pte.is_valid() {
                panic!("kvm_pa: va={:?} mapped pa not valid", va);
            }
            pte.as_phys_addr().as_usize() as u64 + off
        }
        None => {
            panic!("kvm_pa: va={:?} no mapped pa", va);
        }
    }
}
