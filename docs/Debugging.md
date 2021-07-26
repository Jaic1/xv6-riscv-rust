## Debug Story
1. timerinit
2. copy `sie`'s code to `sip`, then clearing `SSIP` becomes clearing `SSIE`,  
which do not allow supervisor software interrupt forever.
3. setting `stvec` the wrong address, supposed to be in virtual space,  
but written as `uservec` directly, which is in physical space.
4. be careful about `pc` when switching page table
5. gdb's watchpoint is useful. For example, watch `$sp` to trace how the stack is used.