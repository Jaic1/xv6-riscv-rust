use crate::consts::memlayout::PHYSTOP;
use crate::mm::PGSIZE;
use core::convert::TryFrom;
use core::result::Result;

#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    /// only accepts addr that is aligned to page size
    pub fn new(addr: usize) -> Result<PhysAddr, &'static str> {
        extern "C" {
            fn end();
        }
        if addr % PGSIZE != 0 {
            return Err("PhysAddr::new: addr not aligned");
        }
        if addr < end as usize || addr > PHYSTOP {
            return Err("PhysAddr::new: addr not in range of memory");
        }
        Ok(PhysAddr(addr))
    }

    /// get the inner addr as usize
    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
    }

    /// consume the PhysAddr to get const ptr
    #[inline]
    pub fn into_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    /// consume the PhysAddr to get mut ptr
    #[inline]
    pub fn into_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
}

/// Wrapper of usize to represent the virtual address
///
/// For 64-bit virtual address, it guarantees that 38-bit to 63-bit are zero
/// reason for 38 instead of 39, from xv6-riscv:
/// one beyond the highest possible virtual address.
/// MAXVA is actually one bit less than the max allowed by
/// Sv39, to avoid having to sign-extend virtual addresses
/// that have the high bit set.
#[repr(transparent)]
pub struct VirtAddr(usize);

impl VirtAddr {
    /// retrieve the vpn\[level\] of the virtual address
    /// only accepts level that is between 0 and 2
    #[inline]
    pub fn page_num(&self, level: usize) -> usize {
        (self.0 >> (12 + level * 9)) & 0x1ff
    }
}

impl TryFrom<usize> for VirtAddr {
    type Error = &'static str;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if (value >> 38) > 0 {
            Err("value for VirtAddr should be smaller than 1<<38")
        } else {
            Ok(Self(value))
        }
    }
}

impl Into<usize> for VirtAddr {
    fn into(self) -> usize {
        self.0
    }
}
