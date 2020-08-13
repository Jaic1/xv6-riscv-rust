#![no_std]
#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(const_in_array_repeat_expressions)]
#![feature(global_asm)]
#![feature(ptr_internals)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
extern crate bitflags;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/kernelvec.S"));
global_asm!(include_str!("asm/swtch.S"));
global_asm!(include_str!("asm/trampoline.S"));

#[macro_use]
pub mod printf;

pub mod console;
pub mod consts;
pub mod fs;
pub mod mm;
pub mod process;
pub mod register;
pub mod rmain;
pub mod spinlock;
pub mod start;
pub mod string;
pub mod trap;
pub mod driver;
pub mod plic;

#[cfg(feature = "unit_test")]
fn test_main_entry() -> ! {
    use process::cpu_id;

    let cpu_id = unsafe { cpu_id() };

    // test cases only needed to be executed with a single hart/kernel-thread
    if cpu_id == 0 {
        spinlock::tests::smoke();           // 1
        process::proc::tests::create();     // 2
        fs::inode::tests::inode_test();     // 3
    }

    // test cases needed to be executed with multiple harts/kernel-threads
    printf::tests::println_simo();          // 4
    mm::kalloc::tests::mm_simo();           // 5
    mm::pagetable::tests::alloc_simo();     // 6
    process::cpu::tests::cpu_id_test();     // 7
    driver::virtio::tests::virtio_simo();   // 8

    if cpu_id == 0 {
        println!("all 8 tests ...pass!");
    }

    loop {}
}
