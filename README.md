# xv6-riscv-rust [TL;DR]
This is a project intending to port xv6-riscv using Rust.  
Most of major differences will be mentioned below,  
while minor differences go to the document/comment in relevant source files.

## Note
1. can only compile on target triple `riscv64gc-unknown-none-elf`,  
refer *.cargo/config* for detail

2. `cargo run` may write things below to the console in the end, which is expected.
```
panickeadni at ckpead niatck 'e'd art 'rrusust_t_mamainin: :enu esd t_main: end of hart 0', src/rmain.rs:34:5
nd of hart 2',o f src/rmain.rs:34:5
hart 1', src/rmain.rs:34:5
```
Reason: there are 3 harts, and `panic` will not acquire the `Pr` lock to write,  
which could be helpful when debugging.

## Usage
Run:
```
cargo run
```
Objdump:
```
cargo objdump --bin xv6-riscv-rust -- -d > kernel.asm

// recommended, some instructions is unknown when using rust-objdump/llvm-objdump
// in target/riscv64gc-unknown-none-elf/debug
riscv64-unknown-elf-objdump -S xv6-riscv-rust > kernel.asm
```
Unit Test:
```
cargo run --features "unit_test"
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

### global_asm
```
global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/kernelvec.S"));
```
xv6-riscv doesn't add `.section .text` in *kernelvec.S*(maybe GCC can infer that),  
but it won't work for xv6-riscv-rust, so we have to explicitly add it.  
You can try it by commenting `.section .text` out and then use objdump to see whether `timervec` exists.

### linker script
```
   .bss : {
     *(.bss)
     *(.sbss*)
-     PROVIDE(end = .);     // works for ld, not for lld
   }
+  PROVIDE(end = .);        // works for lld
```
consider the codes in *kernel.ld* above, see the comments.

### VirtAddr & PhysAddr
when converting usize to VirtAddr & PhysAddr, use from or try_from?  
seems like From and TryFrom can not be both implemented for <T, U>
so I choose to add a new type for trusted address, i.e.,
`impl From<ConstAddr> for VirtAddr` & `impl TryFrom<usize> for VirtAddr`(same for `PhysAddr`)

### sepc
Saving `sepc`(user program counter) in trampoline.S instead of in user_trap.

### RAII lock
Several places deserve to mention:
1. Since we apply RAII-style lock in Rust, so unlike C, some function like  
    `sched` act transparently between the initialization and dropping of the lock/guard.
2. `sleep` method in `Proc` struct receive a spinlock guard, instead of a spinlock,  
    so the caller of this method should reacquire the spinlock if still needed.

### ProcState
`ALLOCATED`:  
this state marks a runnable process is already allocated in a specific cpu,  
in order to temporarily release the proc's lock when  
`CpuManager::scheduler` calls `PROC_MANAGER.alloc_runnable`.

<!-- ### allocate process
`alloc_proc` in `ProcManager` is used to allocate a new process.  
What is different from **xv6-riscv** is that:  
it is passed in a `parent` parameter,  
if `None` => used for the first process to init  
if `Some` => used for existing process to `fork` a child  

**Reason**: Due to In case the parent process is forking a new child process,   -->

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
- [ ] add log layer in fs
- [ ] complete sys_exec and add elf loader
- [ ] complete a runnable fs

## TODO
- [ ] recycle pgt for uvm(no need to recycle pgt for kvm now)
- [ ] remove ConstAddr and PhysAddr?
- [ ] stack size need to be 8192 bytes?
- [ ] meta data portion of buddy system is too high
- [ ] may be too much UB

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

## Story
1. timerinit
2. copy `sie`'s code to `sip`, then clearing `SSIP` becomes clearing `SSIE`,  
which do not allow supervisor software interrupt forever.
3. setting `stvec` the wrong address, supposed to be in virtual space,  
but written as `uservec` directly, which is in physical space.
4. be careful about `pc` when switching page table
