//! Thin helpers over the UEFI text console: clear, colors, and an input reader
//! that accepts keystrokes from EITHER the attached keyboard (UEFI Simple Text
//! Input) OR the COM1 serial line. The dual input means LivingOS is usable both
//! on a physical display and over a serial console (headless).

use alloc::string::String;
use uefi::proto::console::text::{Color, Key, ScanCode};

use crate::serial;

pub fn clear() {
    crate::fbcon::clear();
}

pub fn set_color(fg: Color) {
    let (r, g, b) = match fg {
        Color::Yellow => (240, 220, 90),
        Color::Cyan => (90, 200, 255),
        Color::LightGreen => (120, 220, 150),
        Color::Green => (90, 190, 120),
        Color::Red | Color::LightRed => (235, 95, 95),
        Color::Magenta | Color::LightMagenta => (210, 130, 210),
        Color::LightBlue | Color::Blue => (110, 150, 240),
        Color::White => (240, 244, 255),
        _ => (200, 205, 216),
    };
    crate::fbcon::set_fg(r, g, b);
}

pub fn reset_color() {
    crate::fbcon::set_fg(200, 205, 216);
}

/// Echo a single character to both the display and the serial console.
fn echo(ch: char) {
    crate::kprint!("{}", ch);
}

/// Poll the keyboard for one decoded character. Returns None for keys we don't
/// translate (arrows, F-keys, etc.).
fn poll_keyboard() -> Option<char> {
    let key = uefi::system::with_stdin(|stdin| stdin.read_key().ok().flatten());
    match key {
        Some(Key::Printable(c)) => char::from_u32(u16::from(c) as u32),
        Some(Key::Special(ScanCode::ESCAPE)) => Some('\u{1b}'),
        _ => None,
    }
}

/// Handle one input character. Returns true when the line is complete.
fn handle_char(ch: char, buf: &mut String) -> bool {
    match ch {
        '\r' | '\n' => {
            crate::kprintln!();
            return true;
        }
        '\u{8}' | '\u{7f}' => {
            if buf.pop().is_some() {
                crate::kprint!("\u{8} \u{8}");
            }
        }
        c if (c as u32) >= 0x20 => {
            buf.push(c);
            echo(c);
        }
        _ => {}
    }
    false
}

/// Non-blocking: returns true if a key (keyboard or serial) is waiting,
/// consuming it. Used to hold a view until dismissed.
pub fn any_key() -> bool {
    poll_keyboard().is_some() || serial::try_read_byte().is_some()
}

/// Read one line, blocking, from keyboard or serial. Handles backspace and
/// treats CR or LF as end-of-line. Drains the serial FIFO eagerly each pass so
/// a streamed/pasted line isn't lost to a 16-byte UART overflow.
pub fn read_line() -> String {
    let mut buf = String::new();
    loop {
        let mut progressed = false;
        // Drain everything waiting in the UART receive FIFO before doing
        // anything slow, so bursts don't overflow it.
        while let Some(b) = serial::try_read_byte() {
            progressed = true;
            if handle_char(b as char, &mut buf) {
                return buf;
            }
        }
        // Then one keyboard key per pass.
        if let Some(c) = poll_keyboard() {
            progressed = true;
            if handle_char(c, &mut buf) {
                return buf;
            }
        }
        if !progressed {
            uefi::boot::stall(1_000);
        }
    }
}
