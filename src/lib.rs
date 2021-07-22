#![no_std]
#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(global_asm)]
#![feature(ptr_internals)]
#![feature(const_fn_union)]
#![feature(slice_ptr_get)]
#![feature(new_uninit)]
#![feature(allocator_api)]
#![feature(alloc_error_handler)]
#![allow(dead_code)]
#![warn(rust_2018_idioms)]

#[macro_use]
extern crate bitflags;

extern crate alloc;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/kernelvec.S"));
global_asm!(include_str!("asm/swtch.S"));
global_asm!(include_str!("asm/trampoline.S"));

#[macro_use]
mod printf;

mod consts;
mod fs;
mod mm;
mod process;
mod register;
mod rmain;
mod spinlock;
mod sleeplock;
mod start;
mod trap;
mod driver;
mod plic;

#[cfg(feature = "unit_test")]
fn test_main_entry() {
    use proc::cpu_id;

    let cpu_id = unsafe { cpu_id() };

    // test cases only needed to be executed with a single hart/kernel-thread
    if cpu_id == 0 {
        spinlock::tests::smoke();
    }

    // test cases needed to be executed with multiple harts/kernel-threads
    printf::tests::println_simo();
    mm::kalloc::tests::alloc_simo();

    if cpu_id == 0 {
        println!("all tests pass.");
    }
}
