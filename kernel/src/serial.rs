//! A tiny COM1 (16550 UART) driver so the kernel's console can be mirrored to a
//! serial line. UEFI's own stdout goes to the graphics console; writing to the
//! serial port as well lets us boot headlessly in QEMU (`-serial ...`) and
//! capture exactly what the kernel printed.

use core::arch::asm;
use core::fmt;

const PORT: u16 = 0x3F8; // COM1

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

/// Initialise the UART: 38400 baud, 8N1, FIFO enabled.
pub fn init() {
    unsafe {
        outb(PORT + 1, 0x00); // disable interrupts
        outb(PORT + 3, 0x80); // enable DLAB (set baud divisor)
        outb(PORT + 0, 0x03); // divisor low  (115200 / 3 = 38400)
        outb(PORT + 1, 0x00); // divisor high
        outb(PORT + 3, 0x03); // 8 bits, no parity, one stop bit
        outb(PORT + 2, 0xC7); // enable FIFO, clear, 14-byte threshold
        outb(PORT + 4, 0x0B); // IRQs enabled, RTS/DSR set
    }
}

fn transmit_empty() -> bool {
    unsafe { inb(PORT + 5) & 0x20 != 0 }
}

/// True when the UART has a received byte waiting.
fn data_ready() -> bool {
    unsafe { inb(PORT + 5) & 0x01 != 0 }
}

/// Non-blocking read of one received byte from COM1, if any. This lets the
/// on-OS shell be driven over a serial line (e.g. a headless host or
/// `qemu ... -serial stdio`) in addition to the keyboard.
pub fn try_read_byte() -> Option<u8> {
    if data_ready() {
        Some(unsafe { inb(PORT) })
    } else {
        None
    }
}

fn write_byte(b: u8) {
    while !transmit_empty() {}
    unsafe { outb(PORT, b) }
}

/// A `core::fmt::Write` sink over COM1. `\n` is expanded to `\r\n`.
pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                write_byte(b'\r');
            }
            write_byte(b);
        }
        Ok(())
    }
}
