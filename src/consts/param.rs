/// This is just a maximum used to allocate memory space.
pub const NCPU: usize = 8;

/// Maximum number of processes
pub const NPROC: usize = 64;

/// This is actual number of harts.
/// Same value is passed to qemu with -smp option
pub const NSMP: usize = 3;

pub const CONSOLE_BUF: usize = 128;

/// memory design
pub const PGSIZE: usize = 4096;
pub const PGSHIFT: usize = 12;
pub const PGMASK: usize = 0x1FF;
pub const PGMASKLEN: usize = 9;
