use alloc::sync::Arc;
use core::mem;
use core::num::Wrapping;
use core::sync::atomic::{Ordering, AtomicUsize};
use core::cmp::min;
use core::ptr::addr_of_mut;

use crate::consts::fs::{PIPESIZE, PIPESIZE_U32};
use crate::process::{CPU_MANAGER, PROC_MANAGER};
use crate::spinlock::SpinLock;

use super::{File, FileInner};

#[derive(Debug)]
pub struct Pipe(SpinLock<PipeInner>);

impl Pipe {
    /// Create a [`Pipe`].
    /// Return two files respectively reading from and writing to this [`Pipe`].
    pub fn create() -> Option<(Arc<File>, Arc<File>)> {
        debug_assert!(mem::size_of::<Pipe>() <= 512-2*mem::size_of::<AtomicUsize>());

        // create a pipe
        let mut pipe = Arc::<Self>::try_new_zeroed().ok()?;
        let pipe = unsafe {
            let ptr = Arc::get_mut_unchecked(&mut pipe).as_mut_ptr();
            SpinLock::init_name(addr_of_mut!((*ptr).0), "pipe");
            pipe.assume_init()
        };
        let mut guard = pipe.0.lock();
        guard.read_open = true;
        guard.write_open = true;
        drop(guard);

        // create two files
        let read_file = Arc::try_new(File {
            inner: FileInner::Pipe(Arc::clone(&pipe)),
            readable: true,
            writable: false,
        }).ok()?;
        let write_file = Arc::try_new(File {
            inner: FileInner::Pipe(Arc::clone(&pipe)),
            readable: false,
            writable: true,
        }).ok()?;

        Some((read_file, write_file))
    }

    /// Read from the pipe.
    /// Return the bytes actually read.
    pub(super) fn read(&self, addr: usize, count: u32) -> Result<u32, ()> {
        let p = unsafe { CPU_MANAGER.my_proc() };

        let mut pipe = self.0.lock();

        // wait for data to be written
        while pipe.read_cnt == pipe.write_cnt && pipe.write_open {
            if p.killed.load(Ordering::Relaxed) {
                return Err(())
            }
            p.sleep(&pipe.read_cnt as *const Wrapping<_> as usize, pipe);
            pipe = self.0.lock();
        }

        // read from pipe to user memory
        let count = min(count, (pipe.write_cnt - pipe.read_cnt).0);
        let mut read_count = count;
        for i in 0..count {
            let index = (pipe.read_cnt.0 % PIPESIZE_U32) as usize;
            let byte = pipe.data[index];
            pipe.read_cnt += Wrapping(1);
            if p.data.get_mut().copy_out(&byte as *const u8, addr+(i as usize), 1).is_err() {
                read_count = i;
                break
            }
        }
        unsafe { PROC_MANAGER.wakeup(&pipe.write_cnt as *const Wrapping<_> as usize); }
        drop(pipe);
        Ok(read_count)
    }

    /// Write to the pipe.
    /// Return the bytes actually written.
    pub(super) fn write(&self, addr: usize, count: u32) -> Result<u32, ()> {
        let p = unsafe { CPU_MANAGER.my_proc() };

        let mut pipe = self.0.lock();

        let mut write_count = 0;
        while write_count < count {
            if !pipe.read_open || p.killed.load(Ordering::Relaxed) {
                return Err(())
            }

            if pipe.write_cnt == pipe.read_cnt + Wrapping(PIPESIZE_U32) {
                // wait for data to be read
                unsafe { PROC_MANAGER.wakeup(&pipe.read_cnt as *const Wrapping<_> as usize); }
                p.sleep(&pipe.write_cnt as *const Wrapping<_> as usize, pipe);
                pipe = self.0.lock();
            } else {
                let mut byte: u8 = 0;
                if p.data.get_mut().copy_in(addr+(write_count as usize), &mut byte, 1).is_err() {
                    break;                    
                }
                let i = (pipe.write_cnt.0 % PIPESIZE_U32) as usize;
                pipe.data[i] = byte;
                pipe.write_cnt += Wrapping(1);
                write_count += 1;
            }
        }
        unsafe { PROC_MANAGER.wakeup(&pipe.read_cnt as *const Wrapping<_> as usize); }
        drop(pipe);
        Ok(write_count)
    }

    /// Close one end of the pipe.
    pub(super) fn close(&self, is_write: bool) {
        let mut pipe = self.0.lock();
        if is_write {
            pipe.write_open = false;
            unsafe { PROC_MANAGER.wakeup(&pipe.read_cnt as *const Wrapping<_> as usize); }
        } else {
            pipe.read_open = false;
            unsafe { PROC_MANAGER.wakeup(&pipe.write_cnt as *const Wrapping<_> as usize); }
        }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        debug_assert!({
            let guard = self.0.lock();
            guard.read_open == guard.write_open
        });
    }
}

#[derive(Debug)]
struct PipeInner {
    read_open: bool,
    write_open: bool,
    read_cnt: Wrapping<u32>,
    write_cnt: Wrapping<u32>,
    data: [u8; PIPESIZE],
}
