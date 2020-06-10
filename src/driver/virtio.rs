//! from xv6-riscv:
//! driver for virtio device, only used for disk now

use core::ptr;

use crate::consts::{VIRTIO0, PGSIZE};

/// virtio disk initialization
/// refer detail in virtio version1.1 section3
pub unsafe fn disk_init() {
    if read(VIRTIO_MMIO_MAGIC_VALUE) != 0x74726976 ||
        read(VIRTIO_MMIO_VERSION) != 1 ||
        read(VIRTIO_MMIO_DEVICE_ID) != 2 ||
        read(VIRTIO_MMIO_VENDOR_ID) != 0x554d4551 
    {
        panic!("virtio disk_init: could not find virtio disk");
    }

    // step 1,2,3 - reset and set these two status bit
    let mut status: u32 = 0;
    status |= VIRTIO_CONFIG_S_ACKNOWLEDGE;
    write(VIRTIO_MMIO_STATUS, status);
    status |= VIRTIO_CONFIG_S_DRIVER;
    write(VIRTIO_MMIO_STATUS, status);

    // step 4 - read feature bits and negotiate
    let mut features: u32 = read(VIRTIO_MMIO_DEVICE_FEATURES);
    features &= !(1u32 << VIRTIO_BLK_F_RO);
    features &= !(1u32 << VIRTIO_BLK_F_SCSI);
    features &= !(1u32 << VIRTIO_BLK_F_CONFIG_WCE);
    features &= !(1u32 << VIRTIO_BLK_F_MQ);
    features &= !(1u32 << VIRTIO_F_ANY_LAYOUT);
    features &= !(1u32 << VIRTIO_RING_F_EVENT_IDX);
    features &= !(1u32 << VIRTIO_RING_F_INDIRECT_DESC);
    write(VIRTIO_MMIO_DRIVER_FEATURES, features);

    // step 5
    // set FEATURES_OK bit to tell the device feature negotiation is complete
    status |= VIRTIO_CONFIG_S_FEATURES_OK;
    write(VIRTIO_MMIO_STATUS, status);

    // step 8
    // set DRIVER_OK bit to tell device that driver is ready
    // at this point device is "live"
    status |= VIRTIO_CONFIG_S_DRIVER_OK;
    write(VIRTIO_MMIO_STATUS, status);

    write(VIRTIO_MMIO_GUEST_PAGE_SIZE, PGSIZE as u32);

    // initialize queue 0
    write(VIRTIO_MMIO_QUEUE_SEL, 0);
    let max = read(VIRTIO_MMIO_QUEUE_NUM_MAX);
    if max == 0 {
        panic!("virtio disk has no queue 0");
    }
    if max < NUM {
        panic!("virtio disk max queue short than NUM={}", NUM);
    }
    write(VIRTIO_MMIO_QUEUE_NUM, NUM);

    // TODO - disk page
}

// virtio mmio control registers' offset
// from qemu's virtio_mmio.h
const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004;      // 1 is legacy
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;    // 1: net, 2: disk
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028;   // page size for PFN, write-only
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030; // select queue, write-only
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034; // max size of current queue, read-only
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038; // size of current queue, write-only
const VIRTIO_MMIO_QUEUE_ALIGN: usize = 0x03c;   // used ring alignment, write-only
const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040; // physical page number for queue, read/write
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;   // ready bit
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;  // write-only
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;  // read-only
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064; // write-only
const VIRTIO_MMIO_STATUS: usize = 0x070;

// virtio status register bits
// from qemu's virtio_config.h
const VIRTIO_CONFIG_S_ACKNOWLEDGE: u32 = 1;
const VIRTIO_CONFIG_S_DRIVER: u32 =	2;
const VIRTIO_CONFIG_S_DRIVER_OK: u32 = 4;
const VIRTIO_CONFIG_S_FEATURES_OK: u32 = 8;

// device feature bits
const VIRTIO_BLK_F_RO: u8 = 5;
const VIRTIO_BLK_F_SCSI: u8 = 7;
const VIRTIO_BLK_F_CONFIG_WCE: u8 = 11;
const VIRTIO_BLK_F_MQ: u8 = 12;
const VIRTIO_F_ANY_LAYOUT: u8 = 27;
const VIRTIO_RING_F_INDIRECT_DESC: u8 = 28;
const VIRTIO_RING_F_EVENT_IDX: u8 = 29;

// this many virtio descriptors
// must be a power of 2
const NUM: u32 = 8;

#[inline]
unsafe fn read(offset: usize) -> u32 {
    let src = (Into::<usize>::into(VIRTIO0) + offset) as *const u32;
    ptr::read_volatile(src)
}

#[inline]
unsafe fn write(offset: usize, data: u32) {
    let dst = (Into::<usize>::into(VIRTIO0) + offset) as *mut u32;
    ptr::write_volatile(dst, data);
}

#[repr(C, align(4096))]
struct Disk {
    // TODO
}

#[repr(C)]
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct UsedArea {
    flags: u16,
    id: u16,
    elems: [VRingUsedElem; NUM as usize],
}

#[repr(C)]
struct VRingUsedElem {
    id: u32,
    len: u32,
}
