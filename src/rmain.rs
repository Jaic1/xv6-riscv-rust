/// start() jumps here in supervisor mode on all CPUs.
pub fn rust_main() -> ! {
    crate::console::consoleinit();

    #[cfg(feature = "unit_test")]
    super::test_main_entry();

    panic!("rust_main: end");
}
