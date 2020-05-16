use core::convert::TryFrom;
use core::result::Result;

use crate::consts::{PGMASK, PGMASKLEN, PGSHIFT, PGSIZE, PHYSTOP};

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
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
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

impl TryFrom<usize> for PhysAddr {
    type Error = &'static str;

    fn try_from(addr: usize) -> Result<Self, Self::Error> {
        if addr % PGSIZE != 0 {
            return Err("PhysAddr addr not aligned");
        }
        if addr > PHYSTOP {
            return Err("PhysAddr addr bigger than PHYSTOP");
        }
        Ok(PhysAddr(addr))
    }
}

// impl From<usize> for PhysAddr {
//     fn from(addr: usize) -> Self {
//         Self(addr)
//     }
// }

/// Wrapper of usize to represent the virtual address
///
/// For 64-bit virtual address, it guarantees that 38-bit to 63-bit are zero
/// reason for 38 instead of 39, from xv6-riscv:
/// one beyond the highest possible virtual address.
/// MAXVA is actually one bit less than the max allowed by
/// Sv39, to avoid having to sign-extend virtual addresses
/// that have the high bit set.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
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
        if addr >> 38 != 0 {
            Err("value for VirtAddr should be smaller than 1<<38")
        } else {
            Ok(Self(addr))
        }
    }
}

// impl From<usize> for VirtAddr {
//     fn from(addr: usize) -> Self {
//         Self(addr)
//     }
// }
