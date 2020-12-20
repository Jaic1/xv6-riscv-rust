//! ELF loader

use super::Proc;

/// Load an elf executable into the process's user space
/// note: it can get the mut reference of a Proc,
///     because it will be valid until it calls exit itself
/// TODO
pub fn load(_p: &mut Proc, _path: &[u8]) -> Result<(), &'static str> {
    // get relevant inode using path

    // check elf header, create new empty pagetable for user

    // load each program section

    // allocate space for user stack

    // prepare content in the stack

    // update the process's info

    Ok(())
}