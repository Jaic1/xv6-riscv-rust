/// This is just a maximum used to allocate memory space.
pub const NCPU: usize = 8;
/// This is actual number of harts.
/// Same value is passed to qemu with -smp option
pub const NSMP: usize = 3;
pub const CONSOLE_BUF: usize = 128;
