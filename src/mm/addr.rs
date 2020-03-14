use crate::consts::memlayout::PHYSTOP;
use crate::mm::PGSIZE;
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

#[repr(transparent)]
pub struct VirtAddr(usize);
