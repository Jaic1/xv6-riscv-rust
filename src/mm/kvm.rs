use core::convert::{TryFrom, Into};

use crate::consts::{
    CLINT, CLINT_MAP_SIZE, KERNBASE, PHYSTOP, PLIC, PLIC_MAP_SIZE, UART0, UART0_MAP_SIZE, VIRTIO0,
    VIRTIO0_MAP_SIZE, TRAMPOLINE, PGSIZE
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
        Into::<usize>::into(PHYSTOP) - etext,
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
