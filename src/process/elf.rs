//! ELF loader

use super::Proc;

/// Load an elf executable into the process's user space
/// note: it can get the mut reference of a Proc,
///     because it will be valid until it calls exit itself
pub fn load(p: &mut Proc, path: &[u8]) -> Result<(), &'static str> {
    // get relevant inode using path
    let mut ip = get_inode(path);

    // check elf header, create new empty pagetable for user
    ip.check_header();
    ip.create_pagetable();

    // load each program section
    p.load(&ip.program);

    // update the process's info
    p.update(&ip);

    Ok(())
}