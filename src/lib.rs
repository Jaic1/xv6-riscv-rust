#![no_std]
<<<<<<< HEAD
#![feature(llvm_asm)]
#![feature(global_asm)]
#![feature(const_fn)]
=======
#![feature(llvm_asm)]
#![feature(global_asm)]
>>>>>>> 9c08b3e5498e2cfd8c4ab9361d437c1ed5f7b736
#![feature(const_in_array_repeat_expressions)]
#![feature(global_asm)]
#![feature(ptr_internals)]
#![allow(dead_code)]

#[macro_use]
extern crate bitflags;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/kernelvec.S"));

#[macro_use]
mod printf;

mod console;
mod consts;
mod mm;
mod proc;
mod register;
mod rmain;
mod spinlock;
mod start;
mod string;
mod trap;

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
