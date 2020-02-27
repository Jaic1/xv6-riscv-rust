#![no_std]
#![feature(asm)]
#![feature(global_asm)]
#![allow(dead_code)]

global_asm!(include_str!("asm/entry.S"));

mod console;
mod consts;
#[macro_use]
mod printf;
mod rmain;
mod start;
