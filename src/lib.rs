#![no_std]
#![feature(asm)]
#![feature(global_asm)]
#![feature(const_in_array_repeat_expressions)]
#![feature(ptr_internals)]
#![allow(dead_code)]

#[macro_use]
extern crate bitflags;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/kernelvec.S"));

mod console;
mod consts;
#[macro_use]
mod printf;
mod mm;
mod proc;
mod register;
mod rmain;
mod spinlock;
mod start;
mod string;

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
