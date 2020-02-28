/// start() jumps here in supervisor mode on all CPUs.
pub fn rust_main() -> ! {
    crate::console::consoleinit();
    println!("Hello World");
    panic!("rust_main: end");
}
