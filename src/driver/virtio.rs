//! from xv6-riscv:
//! driver for virtio device, only used for disk now

use array_const_fn_init::array_const_fn_init;

use core::convert::TryFrom;
use core::option::Option;
use core::sync::atomic::{fence, Ordering};
use core::mem;
use core::ptr;

use crate::consts::{PGSHIFT, PGSIZE, VIRTIO0};
use crate::fs::{Buf, BSIZE};
use crate::mm::{kvm_pa, VirtAddr};
use crate::spinlock::SpinLock;
use crate::process::{PROC_MANAGER, CPU_MANAGER};

static mut DISK: Disk = Disk::new();

/// virtio disk initialization
/// refer detail in virtio version1.1 section3
pub unsafe fn disk_init() {
    assert_eq!((&DISK.desc as *const _ as usize) % PGSIZE, 0);
    assert_eq!((&DISK.used as *const _ as usize) % PGSIZE, 0);

    if read(VIRTIO_MMIO_MAGIC_VALUE) != 0x74726976
        || read(VIRTIO_MMIO_VERSION) != 1
        || read(VIRTIO_MMIO_DEVICE_ID) != 2
        || read(VIRTIO_MMIO_VENDOR_ID) != 0x554d4551
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
    if max < NUM as u32 {
        panic!("virtio disk max queue short than NUM={}", NUM);
    }
    write(VIRTIO_MMIO_QUEUE_NUM, NUM as u32);
    let page_num: usize = (&DISK as *const _ as usize) >> PGSHIFT;
    write(VIRTIO_MMIO_QUEUE_PFN, u32::try_from(page_num).unwrap());

    // debug
    println!("virtio disk init: done");
    // TODO - plic.rs and trap.rs arrange for interrupts from VIRTIO0_IRQ
}

pub unsafe fn disk_rw(b: &mut Buf, writing: bool) {
    let sector: u64 = (b.blockno as u64) * (BSIZE as u64 / 512);
    let p = CPU_MANAGER.my_proc();
    let mut guard = DISK.lock.lock();

    // allocate three descriptors
    let mut idx: [usize; 3] = [0; 3];
    loop {
        match alloc3_desc(&mut idx) {
            Ok(_) => break,
            Err(_) => {}
        }
        println!("disk_rw1 goint to sleep!"); // debug
        p.sleep(&DISK.free[0] as *const _ as usize, guard);
        guard = DISK.lock.lock();
        println!("disk_rw1 wakeup!");         // debug
    }

    // format the three descriptors.
    // qemu's virtio-blk.c reads them.
    let buf0 = VirtioBlkOutHdr {
        typed: if writing {
            VIRTIO_BLK_T_OUT
        } else {
            VIRTIO_BLK_T_IN
        },
        reserved: 0,
        sector: sector,
    };

    // buf0 is on a kernel stack, which is not direct mapped,
    // thus the call to kvmpa().
    let buf0_addr = &buf0 as *const _ as usize;
    DISK.desc[idx[0]].addr = kvm_pa(VirtAddr::try_from(buf0_addr).unwrap());
    DISK.desc[idx[0]].len = mem::size_of::<VirtioBlkOutHdr>() as u32;
    DISK.desc[idx[0]].flags = VRING_DESC_F_NEXT;
    DISK.desc[idx[0]].next = idx[1] as u16;

    DISK.desc[idx[1]].addr = b.data.as_ptr() as u64;
    DISK.desc[idx[1]].len = BSIZE as u32;
    DISK.desc[idx[1]].flags = if writing { 0 } else { VRING_DESC_F_WRITE };
    DISK.desc[idx[1]].flags |= VRING_DESC_F_NEXT;
    DISK.desc[idx[1]].next = idx[2] as u16;

    DISK.info[idx[0]].status = 0;
    DISK.desc[idx[2]].addr = &DISK.info[idx[0]].status as *const _ as u64;
    DISK.desc[idx[2]].len = 1;
    DISK.desc[idx[2]].flags = VRING_DESC_F_WRITE;
    DISK.desc[idx[2]].next = 0;

    // record struct buf for virtio_disk_intr().
    b.disk = true;
    DISK.info[idx[0]].b = b as *mut Buf;

    // avail[0] is flags
    // avail[1] tells the device how far to look in avail[2...].
    // avail[2...] are desc[] indices the device should process.
    // we only tell device the first index in our chain of descriptors.
    DISK.avail[2 + (DISK.avail[1] as usize % NUM)] = idx[0] as u16;
    fence(Ordering::SeqCst);
    DISK.avail[1] = DISK.avail[1] + 1;

    write(VIRTIO_MMIO_QUEUE_NOTIFY, 0); // queue 0

    // wait for virtio_disk_intr() to say request has finished.
    while b.disk {
        println!("disk_rw2 goint to sleep!"); // debug
        p.sleep(b as *const _ as usize, guard);
        guard = DISK.lock.lock();
        println!("disk_rw2 wakeup!");         // debug
    }

    DISK.info[idx[0]].b = ptr::null_mut();
    free_chain(idx[0]);

    drop(guard);
}

// find a free descriptor, mark it non-free, return its index.
fn alloc_desc() -> Option<usize> {
    // disk's lock already held
    unsafe {
        for i in 0..NUM {
            if DISK.free[i] {
                DISK.free[i] = false;
                return Some(i);
            }
        }
    }
    None
}

fn alloc3_desc(idx: &mut [usize]) -> Result<(), ()> {
    for i in 0..3 {
        match alloc_desc() {
            Some(ui) => {
                idx[i] = ui;
            }
            None => {
                for j in 0..i {
                    free_desc(j);
                }
                return Err(());
            }
        }
    }
    Ok(())
}

// mark a descriptor as free.
fn free_desc(i: usize) {
    unsafe {
        if i >= NUM {
            panic!("virtio_disk_intr 1");
        }
        if DISK.free[i] {
            panic!("virtio_disk_intr 2");
        }
        DISK.desc[i].addr = 0;
        DISK.free[i] = true;
    }
    // no wakeup
}

// free a chain of descriptors.
fn free_chain(mut i: usize) {
    loop {
        free_desc(i);
        if (unsafe { DISK.desc[i].flags } & VRING_DESC_F_NEXT) > 0 {
            i = unsafe { DISK.desc[i].next } as usize;
        } else {
            break;
        }
    }
}

pub fn disk_intr() {
    unsafe {
        let _lock = DISK.lock.lock();

        while DISK.used_idx % NUM != DISK.used.id as usize % NUM {
            let id = DISK.used.elems[DISK.used_idx].id as usize;

            if DISK.info[id].status != 0 {
                panic!("virtio_disk_intr status");
            }

            if DISK.info[id].b.is_null() {
                panic!("disk_intr: disk's info buf is none");
            } else {
                let bp = DISK.info[id].b;
                (*bp).disk = false;
                PROC_MANAGER.wakeup(bp as usize)
            }

            DISK.used_idx = (DISK.used_idx + 1) % NUM;
        }

        drop(_lock);
    }
}

// virtio mmio control registers' offset
// from qemu's virtio_mmio.h
const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004; // 1 is legacy
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008; // 1: net, 2: disk
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028; // page size for PFN, write-only
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030; // select queue, write-only
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034; // max size of current queue, read-only
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038; // size of current queue, write-only
const VIRTIO_MMIO_QUEUE_ALIGN: usize = 0x03c; // used ring alignment, write-only
const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040; // physical page number for queue, read/write
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044; // ready bit
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050; // write-only
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060; // read-only
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064; // write-only
const VIRTIO_MMIO_STATUS: usize = 0x070;

// virtio status register bits
// from qemu's virtio_config.h
const VIRTIO_CONFIG_S_ACKNOWLEDGE: u32 = 1;
const VIRTIO_CONFIG_S_DRIVER: u32 = 2;
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

// VRingDesc flags
const VRING_DESC_F_NEXT: u16 = 1; // chained with another descriptor
const VRING_DESC_F_WRITE: u16 = 2; // device writes (vs read)

// for disk ops
const VIRTIO_BLK_T_IN: u32 = 0; // read the disk
const VIRTIO_BLK_T_OUT: u32 = 1; // write the disk

// this many virtio descriptors
// must be a power of 2
const NUM: usize = 8;

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
    // a page
    desc: [VRingDesc; NUM],
    avail: [u16; (PGSIZE - NUM * mem::size_of::<VRingDesc>()) / mem::size_of::<u16>()],
    // another page
    used: UsedArea,
    free: [bool; NUM], // TODO - need to start another page?
    used_idx: usize,
    info: [Info; NUM],
    lock: SpinLock<()>,
}

const fn desc_new(_: usize) -> VRingDesc {
    VRingDesc::new()
}

const fn info_new(_: usize) -> Info {
    Info::new()
}

impl Disk {
    const fn new() -> Self {
        Self {
            desc: array_const_fn_init![desc_new; 8],    // 8 is NUM
            avail: [0; (PGSIZE - NUM * mem::size_of::<VRingDesc>()) / mem::size_of::<u16>()],
            used: UsedArea::new(),
            free: [true; NUM],
            used_idx: 0,
            info: array_const_fn_init![info_new; 8],    // 8 is NUM
            lock: SpinLock::new((), "virtio_disk"),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

impl VRingDesc {
    const fn new() -> Self {
        Self {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        }
    }
}

#[repr(C)]
struct UsedArea {
    flags: u16,
    id: u16,
    elems: [VRingUsedElem; NUM],
}

impl UsedArea {
    const fn new() -> Self {
        Self {
            flags: 0,
            id: 0,
            elems: [VRingUsedElem::new(); NUM],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingUsedElem {
    id: u32,
    len: u32,
}

impl VRingUsedElem {
    const fn new() -> Self {
        Self { id: 0, len: 0 }
    }
}

#[repr(C)]
struct VirtioBlkOutHdr {
    typed: u32,
    reserved: u32,
    sector: u64,
}

#[repr(C)]
struct Info {
    b: *mut Buf,
    status: u8,
}

impl Info {
    const fn new() -> Self {
        Self { b: ptr::null_mut(), status: 0 }
    }
}
