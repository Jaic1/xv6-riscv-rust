/// maximum number of device
pub const NDEV: usize = 10;

/// buffer size for console
pub const CONSOLE_BUF: usize = 128;

/// buffer size for uart
pub const UART_BUF: usize = 32;

/// constant device index of console
pub const DEV_CONSOLE: usize = 1;

////////////////////////////////////////////////
///////////    Control Characters   ////////////
////////////////////////////////////////////////

// reference manual: https://man7.org/linux/man-pages/man4/console_codes.4.html

/// end of transmit/file.line
pub const CTRL_EOT: u8 = 0x04;

/// backspace
pub const CTRL_BS: u8 = 0x08;

/// line feed, '\n'
pub const CTRL_LF: u8 = 0x0A;

/// carriage return
pub const CTRL_CR: u8 = 0x0D;

/// DEL
pub const CTRL_DEL: u8 = 0x7f;

/////////////////////////////////////////////////////////////
///////////    Self-defined Control Characters   ////////////
/////////////////////////////////////////////////////////////

/// for debug, print process list
pub const CTRL_PRINT_PROCESS: u8 = 0x10;

/// backspace the whole line
// TODO
pub const CTRL_BS_LINE: u8 = 0x15;
