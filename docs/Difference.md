## Difference
no longer maintained

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

### Buddy System Allocator
We have replaced the linked list allocator with buddy system allocator.