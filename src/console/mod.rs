mod uart;

// fn consputbs() {
//     // b'\b' not supported in rust
//     const BACKSPACE: u8 = 8;
//     uart::uartputc(BACKSPACE);
//     uart::uartputc(b' ');
//     uart::uartputc(BACKSPACE);
// }

pub fn consputc(c: u8) {
    uart::uartputc(c);
}

pub fn consoleinit() {
    uart::uartinit();
}
