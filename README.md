# xv6-riscv-rust [TL;DR]
This is a project intending to port xv6-riscv using Rust.  
Some major differences will be mentioned below.

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
Verbose debug info:
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
- [ ] add sys_open, sys_read, sys_write
- [ ] complete a runnable fs

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

## Debug Story
1. timerinit
2. copy `sie`'s code to `sip`, then clearing `SSIP` becomes clearing `SSIE`,  
which do not allow supervisor software interrupt forever.
3. setting `stvec` the wrong address, supposed to be in virtual space,  
but written as `uservec` directly, which is in physical space.
4. be careful about `pc` when switching page table

5. gdb's watchpoint is useful. For example, watch `$sp` to trace how the stack is used.

First, I mistakenly set `kstack` to `pa`(should be `va`, in *proc_init fn*), which cause `kstack` conflicts with the heap,  
more specifically, the heap memory holding a user's third-level pagetale.  
And this pagetable map `trampoline`, so after the kernel taint the content through `kstack`, the user will cause *instruction fault* when it traps, because it will traps to `trampoline` first, but its instructions are tainted.

Second, I increment the very-initial kernel stack from 4096 to 8192.

Third, the previous kstack size is 4096 bytes, which is not enough for some Rust codes, especially in debug mode.  
In `sys_exec`:
```
fn sys_exec(&mut self) -> SysResult {
    let mut path: [u8; MAXPATH] = [0; MAXPATH];
    if let Err(s) = self.arg_str(0, &mut path) {
        syscall_warning(s);
        return Err(())
    }
}
```
Assembly:
```
Disassembly of section .text._ZN81_$LT$rix..process..proc..Proc$u20$as$u20$rix..process..proc..syscall..Syscall$GT$8sys_exec17hb480b32dbb1dd6c4E:

0000000080005138 <_ZN81_$LT$rix..process..proc..Proc$u20$as$u20$rix..process..proc..syscall..Syscall$GT$8sys_exec17hb480b32dbb1dd6c4E>:
pub trait Syscall {
    fn sys_exec(&mut self) -> SysResult;
}

impl Syscall for Proc {
    fn sys_exec(&mut self) -> SysResult {
    80005138:	81010113          	addi	sp,sp,-2032
    8000513c:	7e113423          	sd	ra,2024(sp)
    80005140:	6585                	lui	a1,0x1
    80005142:	fc05859b          	addiw	a1,a1,-64
    80005146:	40b10133          	sub	sp,sp,a1
    8000514a:	fe2a                	sd	a0,312(sp)
    8000514c:	6585                	lui	a1,0x1
    8000514e:	3e85859b          	addiw	a1,a1,1000
    80005152:	958a                	add	a1,a1,sp
    80005154:	e188                	sd	a0,0(a1)
    80005156:	0a88                	addi	a0,sp,336
    80005158:	f62a                	sd	a0,296(sp)
    8000515a:	4581                	li	a1,0
    ......
```
Solution1: increase the kstack size to 8192/16384 bytes.  
Solution2: look through the code, many times there are some code patterns that can make Rustc allocate too much stack space.  
We can also dump the kernel code to asm to check which funtion use many stack space, for example, use `sp,sp,-[0-9]{4}` regular expression to search in vscode.  
Solution3: switch to release mode, haha.  

Finally, I choose the kernel stack size to be 4096B*4=16384B=16KB, which is temporarily enough for debug mode.
