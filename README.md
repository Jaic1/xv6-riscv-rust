# xv6-riscv-rust
This is a project intending to port xv6-riscv using Rust.  
Some difference will be mentioned below.

## Note
can only compile on target triple `riscv64gc-unknown-none-elf`,  
refer *.cargo/config* for detail

## Difference
1. while xv6-riscv wraps spinlock into a single object,  
xv6-riscv-rust wrap a single object into a mutex, which implemented with spinlock.

## Path
- [x] porting console and uart to support printf, p.s., smp = 1
- [x] add register abstraction to support start using mret to return to rust_main
- [x] cpu abstraction and spinlock, add unit_test feature as temp solution

## TODO
