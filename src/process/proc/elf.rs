//! ELF loader

use alloc::boxed::Box;
use alloc::str;
use core::{cmp::min, convert::TryFrom, mem::{self, MaybeUninit}};

use crate::{consts::{MAXARGLEN, PGSIZE, MAXARG}, sleeplock::SleepLockGuard};
use crate::mm::{Address, PageTable, Addr, VirtAddr, pg_round_up};
use crate::fs::{ICACHE, Inode, LOG, InodeData};

use super::Proc;

/// Load an elf executable into the process's user space.
pub fn load(p: &mut Proc, path: &[u8], argv: &[Option<Box<[u8; MAXARGLEN]>>]) -> Result<usize, &'static str> {
    // get relevant inode using path
    let inode: Inode;
    LOG.begin_op();
    match ICACHE.namei(path) {
        Some(i) => inode = i,
        None => {
            LOG.end_op();
            return Err("cannot name inode")
        },
    }

    // check elf header
    // create a new empty pagetable, but not assign yet
    let mut idata = inode.lock();
    let mut elf = MaybeUninit::<ElfHeader>::uninit();
    if idata.iread(
        Address::KernelMut(elf.as_mut_ptr() as *mut u8),
        0, 
        mem::size_of::<ElfHeader>() as u32
    ).is_err() {
        drop(idata); drop(inode); LOG.end_op();
        return Err("cannot read elf inode")
    }
    let elf = unsafe { elf.assume_init() };
    if elf.magic != ELF_MAGIC {
        drop(idata); drop(inode); LOG.end_op();
        return Err("bad elf magic number")
    }

    // allocate new pagetable, not assign to proc yet
    let pdata = p.data.get_mut();
    let mut pgt;
    match PageTable::alloc_proc_pagetable(pdata.tf as usize) {
        Some(p) => pgt = p,
        None => {
            drop(idata); drop(inode); LOG.end_op();
            return Err("mem not enough")
        },
    }
    let mut proc_size = 0usize;

    // load each program section
    let ph_size = mem::size_of::<ProgHeader>() as u32;
    let mut off = elf.phoff as u32;
    for _ in 0..elf.phnum {
        let mut ph = MaybeUninit::<ProgHeader>::uninit();
        if idata.iread(Address::KernelMut(ph.as_mut_ptr() as *mut u8), off, ph_size).is_err() {
            pgt.dealloc_proc_pagetable(proc_size);
            drop(pgt); drop(idata); drop(inode); LOG.end_op();
            return Err("cannot read elf program header")
        }
        let ph = unsafe { ph.assume_init() };
        
        if ph.pg_type != ELF_PROG_LOAD {
            off += ph_size;
            continue;
        }

        if ph.memsz < ph.filesz || ph.vaddr + ph.memsz < ph.vaddr || ph.vaddr % (PGSIZE as u64) != 0 {
            pgt.dealloc_proc_pagetable(proc_size);
            drop(pgt); drop(idata); drop(inode); LOG.end_op();
            return Err("one program header meta not correct")
        }

        match pgt.uvm_alloc(proc_size, (ph.vaddr + ph.memsz) as usize) {
            Ok(cur_size) => proc_size = cur_size,
            Err(_) => {
                pgt.dealloc_proc_pagetable(proc_size);
                drop(pgt); drop(idata); drop(inode); LOG.end_op();
                return Err("not enough uvm for program header")
            }
        }

        if load_seg(pgt.as_mut(), ph.vaddr as usize, &mut idata, ph.off as u32, ph.filesz as u32).is_err() {
            pgt.dealloc_proc_pagetable(proc_size);
            drop(pgt); drop(idata); drop(inode); LOG.end_op();
            return Err("load program section error")
        }

        off += ph_size;
    }
    drop(idata);
    drop(inode);
    LOG.end_op();

    // allocate two page for user stack
    // one for usage, the other for guarding
    proc_size = pg_round_up(proc_size);
    match pgt.uvm_alloc(proc_size, proc_size + 2*PGSIZE) {
        Ok(ret_size) => proc_size = ret_size,
        Err(_) => {
            pgt.dealloc_proc_pagetable(proc_size);
            return Err("not enough uvm for user stack")
        },
    }
    pgt.uvm_clear(proc_size - 2*PGSIZE);
    let mut stack_pointer = proc_size;
    let stack_base = stack_pointer - PGSIZE;

    // prepare command line content in the user stack
    let argc = argv.len();
    debug_assert!(argc < MAXARG);
    let mut ustack = [0usize; MAXARG+1];
    for i in 0..argc {
        let arg_slice = argv[i].as_deref().unwrap();
        let max_pos = arg_slice.iter().position(|x| *x==0).unwrap();
        let count = max_pos + 1;    // counting the ending zero
        stack_pointer -= count;
        stack_pointer = align_sp(stack_pointer);
        if stack_pointer < stack_base {
            pgt.dealloc_proc_pagetable(proc_size);
            return Err("cmd args too much for stack")
        }
        if pgt.copy_out(arg_slice.as_ptr(), stack_pointer, count).is_err() {
            pgt.dealloc_proc_pagetable(proc_size);
            return Err("copy cmd args to pagetable go wrong")
        }
        ustack[i] = stack_pointer;
    }
    debug_assert!(argc == 0 || ustack[argc-1] != 0);    // ustack[argc-1] should not be zero
    debug_assert_eq!(ustack[argc], 0);                  // ustack[argc] should be zero
    stack_pointer -= (argc+1) * mem::size_of::<usize>();
    stack_pointer = align_sp(stack_pointer);
    if stack_pointer < stack_base {
        pgt.dealloc_proc_pagetable(proc_size);
        return Err("cmd args too much for stack")
    }
    if pgt.copy_out(ustack.as_ptr() as *const u8, stack_pointer, (argc+1)*mem::size_of::<usize>()).is_err() {
        pgt.dealloc_proc_pagetable(proc_size);
        return Err("copy cmd args to pagetable go wrong")
    }

    // update the process's info
    let tf = unsafe { pdata.tf.as_mut().unwrap() };
    tf.a1 = stack_pointer;
    let off = path.iter().position(|x| *x!=b'/').unwrap();
    let count = min(path.len()-off, pdata.name.len());
    for i in 0..count {
        pdata.name[i] = path[i+off];
    }
    let mut old_pgt = pdata.pagetable.replace(pgt).unwrap();
    let old_size = pdata.sz;
    pdata.sz = proc_size;
    tf.epc = elf.entry as usize;
    tf.sp = stack_pointer;
    old_pgt.dealloc_proc_pagetable(old_size);
    
    Ok(argc)
}

/// Load a program segment into the user's virtual memory.
/// Note: va should be page-aligned and [va, offset+size) should already be mapped.
fn load_seg(pgt: &mut PageTable, va: usize, idata: &mut SleepLockGuard<'_, InodeData>, offset: u32, size: u32)
    -> Result<(), ()>
{
    if va % PGSIZE != 0 {
        panic!("va={} is not page aligned", va);
    }
    let mut va = VirtAddr::try_from(va).unwrap();

    for i in (0..size).step_by(PGSIZE) {
        let pa: usize;
        match pgt.walk_addr_mut(va) {
            Ok(phys_addr) => pa = phys_addr.into_raw(),
            Err(s) => panic!("va={} should already be mapped, {}", va.into_raw(), s),
        }
        let count = if size - i < (PGSIZE as u32) {
            size - i
        } else {
            PGSIZE as u32
        };
        if idata.iread(Address::KernelMut(pa as *mut u8), offset+i, count).is_err() {
            return Err(())
        }
        va.add_page();
    }

    Ok(())
}

#[inline(always)]
fn align_sp(sp: usize) -> usize {
    sp - (sp % 16)
}

#[repr(C)]
struct ElfHeader {
    magic: u32,
    elf: [u8; 12],
    elf_type: u16,
    machine: u16,
    version: u32,
    entry: u64,
    /// offset of program headers
    phoff: u64,
    shoff: u64,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    /// number of program headers
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

#[repr(C)]
struct ProgHeader {
    pg_type: u32,
    flags: u32,
    off: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

const ELF_MAGIC: u32 = 0x464C457F;
const ELF_PROG_LOAD: u32 = 1;
