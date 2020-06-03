use super::*;

/// RV64 Sv39 Scheme

/// lower flag bits length
pub const SV39FLAGLEN: usize = 10;
/// scheme flag
pub const SATP_SV39: usize = 8usize << 60;

/// highest possible virtual address
/// one bit less than the maximum allowed by Sv39
pub const MAXVA: ConstAddr = ConstAddr(1usize << (9 + 9 + 9 + 12 - 1));
