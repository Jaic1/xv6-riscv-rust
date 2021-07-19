use core::convert::TryFrom;
use core::result::Result;
use core::ops::{Add, Sub};

use crate::consts::{PGMASK, PGMASKLEN, PGSHIFT, PGSIZE, PHYSTOP, MAXVA, ConstAddr};

pub trait Addr {
    fn data_ref(&self) -> &usize;

    fn data_mut(&mut self) -> &mut usize;

    #[inline]
    fn pg_round_up(&mut self) {
        *self.data_mut() = (*self.data_mut() + PGSIZE - 1) & !(PGSIZE - 1)
    }

    #[inline]
    fn pg_round_down(&mut self) {
        *self.data_mut() = *self.data_mut() & !(PGSIZE - 1)
    }

    #[inline]
    fn add_page(&mut self) {
        *self.data_mut() += PGSIZE;
    }

    #[inline]
    fn as_usize(&self) -> usize {
        *self.data_ref()
    }

    #[inline]
    fn as_ptr(&self) -> *const u8 {
        *self.data_ref() as *const u8
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        *self.data_mut() as *mut u8
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PhysAddr(usize);

impl Addr for PhysAddr {
    #[inline]
    fn data_ref(&self) -> &usize {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl PhysAddr {
    /// Construct a [`PhysAddr`] from a trusted usize.
    /// SAFETY: The caller should ensure that the raw usize is acutally a valid [`PhysAddr`].
    #[inline]
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    /// Leak the [`PhysAddr`]'s inner address.
    #[inline]
    pub fn into_raw(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for PhysAddr {
    type Error = &'static str;

    fn try_from(addr: usize) -> Result<Self, Self::Error> {
        if addr % PGSIZE != 0 {
            return Err("PhysAddr addr not aligned");
        }
        if addr > usize::from(PHYSTOP) {
            return Err("PhysAddr addr bigger than PHYSTOP");
        }
        Ok(PhysAddr(addr))
    }
}

impl From<ConstAddr> for PhysAddr {
    fn from(const_addr: ConstAddr) -> Self {
        Self(const_addr.into())
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
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct VirtAddr(usize);

impl Addr for VirtAddr {
    #[inline]
    fn data_ref(&self) -> &usize {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl VirtAddr {
    /// Construct a [`VirtAddr`] from a trusted usize.
    /// SAFETY: The caller should ensure that the raw usize is acutally a valid [`VirtAddr`].
    #[inline]
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    /// Leak the [`VirtAddr`]'s inner address.
    #[inline]
    pub fn into_raw(self) -> usize {
        self.0
    }

    /// retrieve the vpn\[level\] of the virtual address
    /// only accepts level that is between 0 and 2
    #[inline]
    pub fn page_num(&self, level: usize) -> usize {
        (self.0 >> (PGSHIFT + level * PGMASKLEN)) & PGMASK
    }
}

impl TryFrom<usize> for VirtAddr {
    type Error = &'static str;

    fn try_from(addr: usize) -> Result<Self, Self::Error> {
        if addr > MAXVA.into() {
            Err("value for VirtAddr should be smaller than 1<<38")
        } else {
            Ok(Self(addr))
        }
    }
}

impl From<ConstAddr> for VirtAddr {
    fn from(const_addr: ConstAddr) -> Self {
        Self(const_addr.into())
    }
}

impl Add for VirtAddr {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for VirtAddr {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}
