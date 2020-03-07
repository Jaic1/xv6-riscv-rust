# xv6-riscv-rust
This is a project intending to port xv6-riscv using Rust.  
Most of major differences will be mentioned below,  
while minor differences go to the document/comment in relevant source files.

## Note
1. can only compile on target triple `riscv64gc-unknown-none-elf`,  
refer *.cargo/config* for detail

## Usage
Run:
```
cargo run
```
Get assembly code of xv6-riscv-rust:
```
cargo objdump --bin xv6-riscv-rust -- -d > kernel.asm

// recommended, some instructions is unknown when using rust-objdump/llvm-objdump
// in target/riscv64gc-unknown-none-elf/debug
riscv64-unknown-elf-objdump -S xv6-rsicv > kernel.asm
```
Unit Test:
```
cargo run --features "unit_test"
```
Objdump:
```
cargo objdump --bin xv6-riscv-rust -- -d > kernel.asm
```
target spec:
```
rustc -Z unstable-options --print target-spec-json --target riscv64gc-unknown-none-elf
```

## Difference

### SpinLock
1. while xv6-riscv wraps spinlock into a single object,  
    xv6-riscv-rust wrap a single object into a mutex, which implemented with spinlock.
2. while xv6-riscv use `initlock` to init lock at run time,  
    xv6-riscv-rust use const fn `SpinLock::new` to init lock at compile time.

### AtomicBool for simple symbol
xv6-riscv-rust use AtomicBool to replace simple int symbol in xv6-riscv,  
in cases when these symbols are only written once or are rarely used.  
Reason: safer, don't lose much speed.  

1. `STARTED` in rmain.rs:  
    It is written once by hart0 to tell other harts that some initialization done.
2. `Pr.locking` in printf.rs:  
    It is written once by the thread that panic.

### amoswap and lr&sc
GCC's `__sync_lock_test_and_set` generate `amoswap`,  
while Rust's `compare_and_swap` / LLVM's `cmpxchg` generate `lr`&`sc`.  
Helpful reference: [Rust Atomic compare and swap 2018 editionのRISC-Vソース〜LLVMを添えて〜](https://qiita.com/tomoyuki-nakabayashi/items/1ec7e075d4417c1a1fbe#dive-into-the-llvm-ir)

### Unit Test
xv6-riscv-rust use conditional compilation trick to implement some unit tests.  
Test cases will go in submodule in several mods that is needed to be tested,  
typically named `pub mod tests` in each mod.  
Example:
```
#[cfg(feature = "unit_test")]
pub mod tests {
    use super::*;

    /* test cases */
}
```
With the feature unit_test enabled,  
here is the execution flow: after each hart enter and execute `rust_main`,  
they will call `test_main_entry`, which call each tests submodule.  
Usage: add cargo options `--features "unit_test`

## Path
- [x] porting console and uart to support printf, p.s., smp = 1
- [x] add register abstraction to support start using mret to return to rust_main
- [x] cpu abstraction and spinlock, add unit_test feature as temp solution
- [x] us spin e lock to synchronize con print sole's ln, and refactor PRINT
- [ ] kvm
- [ ] add more to console, i.e., consoleread, consolewrite, console

## TODO
- [ ] use form or table in Difference section
- [ ] add travis CI

## Useful Reference
[Why implementing Send trait for Mutex?](https://users.rust-lang.org/t/why-we-implement-send-trait-for-mutex/39065)  
[Explicitly drop](https://users.rust-lang.org/t/is-this-piece-of-codes-in-good-style/39095)