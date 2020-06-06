use core::ptr;
use core::option::Option;
use core::convert::TryFrom;

use crate::consts::{TRAMPOLINE, TRAPFRAME, PGSIZE};
use crate::spinlock::SpinLock;
use crate::mm::{Box, PageTable, VirtAddr, PhysAddr, PteFlag};

use super::{Context, TrapFrame, fork_ret};

#[derive(Eq, PartialEq, Debug)]
pub enum ProcState { UNUSED, SLEEPING, RUNNABLE, RUNNING, ZOMBIE }

pub struct Proc {
    pub lock: SpinLock<()>,

    // p->lock must be held when using these:
    pub state: ProcState,
    pub pid: usize,

    // lock need not be held, or
    // lock already be held
    kstack: usize,
    pagetable: Option<Box<PageTable>>,
    tf: *mut TrapFrame,
    context: Context,
}

impl Proc {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new((), "proc"),
            state: ProcState::UNUSED,
            pid: 0,
            kstack: 0,
            pagetable: None,
            tf: ptr::null_mut(),
            context: Context::new(),
        }
    }

    pub fn set_kstack(&mut self, kstack: usize) {
        self.kstack = kstack;
    }

    /// Allocate a new user pagetable for itself
    /// and map trampoline code and trapframe
    pub fn proc_pagetable(&mut self) {
        extern "C" {
            fn trampoline();
        }

        let mut pagetable = PageTable::uvm_create();
        pagetable.map_pages(VirtAddr::from(TRAMPOLINE), PGSIZE,
            PhysAddr::try_from(trampoline as usize).unwrap(), PteFlag::R | PteFlag::X)
            .expect("user proc table mapping trampoline");
        pagetable.map_pages(VirtAddr::from(TRAPFRAME), PGSIZE,
            PhysAddr::try_from(self.tf as usize).unwrap(), PteFlag::R | PteFlag::W)
            .expect("user proc table mapping trapframe");

        self.pagetable = Some(pagetable);
    }

    pub fn set_tf(&mut self, tf: *mut TrapFrame) {
        self.tf = tf;
    }

    /// Init the context of the process after it is created
    /// Set its return address to fork_ret,
    /// which start to return to user space.
    pub fn init_context(&mut self) {
        self.context.clear();
        self.context.set_ra(fork_ret as *const () as usize);
        self.context.set_sp(self.kstack + PGSIZE);
    }

    /// Return the process's mutable reference of context
    pub fn get_context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Called by ProcManager's user_init,
    /// only be called once
    /// TODO - copy used code and sth else
    pub fn user_init(&mut self) {
        self.state = ProcState::RUNNABLE;
    }
}
