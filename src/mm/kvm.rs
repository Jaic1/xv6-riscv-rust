use core::convert::TryFrom;

use crate::consts::{
    CLINT, CLINT_MAP_SIZE, KERNBASE, PHYSTOP, PLIC, PLIC_MAP_SIZE, UART0, UART0_MAP_SIZE, VIRTIO0,
    VIRTIO0_MAP_SIZE,
};
use crate::mm::{Addr, PageTable, PhysAddr, PteFlag, VirtAddr};
use crate::register::satp;

static mut KERNEL_PAGE_TABLE: PageTable = PageTable::empty();

pub unsafe fn kvm_init_hart() {
    satp::write(KERNEL_PAGE_TABLE.as_satp());
    llvm_asm!("sfence.vma zero, zero"::::"volatile");
}

pub unsafe fn kvm_init() {
    // uart registers
    kvm_map(
        VirtAddr::try_from(UART0).unwrap(),
        PhysAddr::try_from(UART0).unwrap(),
        UART0_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // virtio mmio disk interface
    kvm_map(
        VirtAddr::try_from(VIRTIO0).unwrap(),
        PhysAddr::try_from(VIRTIO0).unwrap(),
        VIRTIO0_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // CLINT
    kvm_map(
        VirtAddr::try_from(CLINT).unwrap(),
        PhysAddr::try_from(CLINT).unwrap(),
        CLINT_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    // PLIC
    kvm_map(
        VirtAddr::try_from(PLIC).unwrap(),
        PhysAddr::try_from(PLIC).unwrap(),
        PLIC_MAP_SIZE,
        PteFlag::R | PteFlag::W,
    );

    extern "C" {
        fn etext();
    }
    let etext = etext as usize;

    // map kernel text executable and read-only.
    kvm_map(
        VirtAddr::try_from(KERNBASE).unwrap(),
        PhysAddr::try_from(KERNBASE).unwrap(),
        etext - KERNBASE,
        PteFlag::R | PteFlag::X,
    );

    // map kernel data and the physical RAM we'll make use of.
    kvm_map(
        VirtAddr::try_from(etext).unwrap(),
        PhysAddr::try_from(etext).unwrap(),
        PHYSTOP - etext,
        PteFlag::R | PteFlag::W,
    );

    // TODO
    // map the trampoline for trap entry/exit to
    // the highest virtual address in the kernel.
    // kvmmap(TRAMPOLINE, (uint64)trampoline, PGSIZE, PTE_R | PTE_X);
}

pub unsafe fn kvm_map(va: VirtAddr, pa: PhysAddr, size: usize, perm: PteFlag) {
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
