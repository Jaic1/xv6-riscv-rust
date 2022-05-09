# xv6-riscv-rust
This is a project intending to port xv6-riscv using Rust.  
It is now runnable.

## Usage
0. Follow [6.S081 2021](https://pdos.csail.mit.edu/6.828/2021/tools.html)/[6.S081 2020](https://pdos.csail.mit.edu/6.828/2020/tools.html) to install tools.

    We may need to build qemu from source depending on the machine.

1. Build fs:
```
make fs.img
```
2. Run:
```
cargo run
```

## Misc Options/Features
Objdump:
```
cargo objdump --bin xv6-riscv-rust -- -d > kernel.asm

// recommended, some instructions is unknown when using rust-objdump/llvm-objdump
// in target/riscv64gc-unknown-none-elf/debug
riscv64-unknown-elf-objdump -S xv6-riscv-rust > kernel.asm
```
trace system call:
```
cargo run --features "trace_syscall"
```
Verbose init info:
```
cargo run --features "verbose_init_info"
```
Unit Test(deprecated):
```
cargo run --features "unit_test"
```
target spec:
```
rustc -Z unstable-options --print target-spec-json --target riscv64gc-unknown-none-elf
```

## Path
- [x] porting console and uart to support printf, p.s., smp = 1
- [x] add register abstraction to support start using mret to return to rust_main
- [x] cpu abstraction and spinlock, add unit_test feature as temp solution
- [x] us spin e lock to synchronize con print sole's ln, and refactor PRINT
- [x] add kernel frame allocator(kalloc), fix writing bug in `timerinit`
- [x] use [Unique](https://doc.rust-lang.org/1.26.2/std/ptr/struct.Unique.html) in self-implemented Box to provide ownership, see [this](https://doc.rust-lang.org/nomicon/vec-layout.html) for example
- [x] add Addr and PageTable
- [x] add kvm for kernel, i.e., kernel paging
- [x] cpu and proc basic abstraction(hard time playing around lock and borrow checker)
- [x] add kernel trap handler(panic at `fork_ret`)
- [x] add user trap returner and way to user space
- [x] add user code space(initcode) and ecall handing in `user_trap`
- [x] add virtio disk driver, plic, buffer cache, inode
- [x] refactor `Proc` into several parts, one need lock to protect, the other is private
- [x] separate `Buf` into two parts, one guarded by bcache's lock, the guarded by its own sleeplock
- [x] update bio and virtio disk
- [x] replace linked list allocator with buddy system, remove self-implemented Box
- [x] add log layer in fs
- [x] add inode layer in fs
- [x] complete sys_exec and add elf loader
- [x] add console, refactor uart and print
- [x] add file layer and sys_open, sys_dup, sys_write
- [x] add several sys_* func
- [x] add pipe in fs and also sys_unlink, sys_chdir, sys_pipe
- [x] port user library
- [x] add several sys_* func and handle some OOM cases
- [x] enable all harts

## TODO
- [ ] recycle pgt for uvm(no need to recycle pgt for kvm now)
- [ ] remove ConstAddr and PhysAddr?
- [ ] stack size need to be 8192 bytes?
- [ ] meta data portion of buddy system is too high
- [ ] may be too much UB
- [ ] one-time init, like Once
- [ ] some assertions can switch to debug_assert, compile time assert
- [ ] remove `VirtAddr` and `PhysAddr`
- [ ] refactor superblock
- [ ] refactor `begin_op` and `end_op`
- [ ] compare raw pointer's `get_mut` method with null-unchecked version `&mut *`
- [ ] [new_uninit](https://github.com/rust-lang/rust/issues/63291)
- [ ] OOM
- [ ] unexpected external interrupt irq=0

## Useful Reference
[Why implementing Send trait for Mutex?](https://users.rust-lang.org/t/why-we-implement-send-trait-for-mutex/39065)  
[Explicitly drop](https://users.rust-lang.org/t/is-this-piece-of-codes-in-good-style/39095)  
[fixed-size linked list allocator](https://users.rust-lang.org/t/how-to-implement-a-single-linked-list-in-os-bare-metal/39223)  
[take ownership from nothing](https://stackoverflow.com/questions/57225328/how-to-take-ownership-of-a-c-pointer-in-rust-and-drop-it-appropriately)  
[Unique issue](https://www.reddit.com/r/rust/comments/bcb0dh/replacement_for_stdptrunique_and_stdptrshared/)  
[out of memory](https://www.reddit.com/r/rust/comments/279k7i/whats_rusts_mechanism_for_recovering_from_say/)  
[integrate Mutex and MutexGuard](https://users.rust-lang.org/t/integrate-mutex-and-mutexguard-into-a-struct/43735)  
[lld linker script](https://sourceware.org/binutils/docs/ld/Scripts.html)  
[Rust Memory layout](https://docs.rust-embedded.org/embedonomicon/memory-layout.html)  
[rustc codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html)  
[Consider deprecation of UB-happy static mut](https://github.com/rust-lang/rust/issues/53639)  
[non-reentrant function](https://doc.bccnsoft.com/docs/rust-1.36.0-docs-html/embedded-book/start/exceptions.html)  
[Cpp's Relaxed ordering](https://en.cppreference.com/w/cpp/atomic/memory_order#Relaxed_ordering)  
[Rust build profile](https://doc.rust-lang.org/cargo/reference/profiles.html)  
