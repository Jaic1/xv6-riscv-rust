//! string module containing C-like mem operation, like memset
//! Is there a better way to rewrite these ops in Rust?

/// memset
///
/// only support [u8]
pub unsafe fn memset(dst: *mut u8, c: u8, n: usize) -> *mut u8 {
    for _ in 0..n {
        *dst = c;
    }
    dst
}
