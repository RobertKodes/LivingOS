//! Framebuffer text console.
//!
//! Renders the shell directly to the GPU framebuffer so the *entire* LivingOS
//! session — boot, the society, goals, the audit trail — is visible in the
//! display, not only on the serial line. This is the first piece of the visual
//! GUI: a console drawn with our own bitmap font over the GOP framebuffer.

use crate::font::FONT;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

const CW: usize = 8; // glyph cell width
const CH: usize = 16; // glyph cell height

static mut BASE: u64 = 0;
static mut W: usize = 0;
static mut H: usize = 0;
static mut STRIDE: usize = 0;
static mut BGR: bool = true;
static mut COL: usize = 0;
static mut ROW: usize = 0;
static mut FG: u32 = 0;
static mut READY: bool = false;

fn pack(r: u8, g: u8, b: u8) -> u32 {
    unsafe {
        if BGR {
            ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        } else {
            ((b as u32) << 16) | ((g as u32) << 8) | r as u32
        }
    }
}

fn bg() -> u32 {
    pack(10, 12, 18)
}

unsafe fn cols() -> usize {
    if W == 0 { 0 } else { W / CW }
}
unsafe fn rows() -> usize {
    if H == 0 { 0 } else { H / CH }
}

/// Grab the GOP framebuffer (while boot services are active) and clear it.
pub fn init() {
    if let Ok(h) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        if let Ok(mut gop) = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(h) {
            let info = gop.current_mode_info();
            let (w, hh) = info.resolution();
            let stride = info.stride();
            let is_bgr = !matches!(info.pixel_format(), PixelFormat::Rgb);
            let mut fb = gop.frame_buffer();
            unsafe {
                BASE = fb.as_mut_ptr() as u64;
                W = w;
                H = hh;
                STRIDE = stride;
                BGR = is_bgr;
                FG = pack(200, 205, 216);
                READY = true;
            }
        }
    }
    clear();
}

pub fn available() -> bool {
    unsafe { READY }
}

pub fn set_fg(r: u8, g: u8, b: u8) {
    unsafe {
        if READY {
            FG = pack(r, g, b);
        }
    }
}

pub fn clear() {
    unsafe {
        if !READY {
            return;
        }
        let base = BASE as *mut u32;
        let n = STRIDE * H;
        let c = bg();
        let mut i = 0;
        while i < n {
            base.add(i).write_volatile(c);
            i += 1;
        }
        COL = 0;
        ROW = 0;
    }
}

unsafe fn glyph(col: usize, row: usize, ch: u8) {
    let gi = (ch as usize).wrapping_sub(0x20);
    let g = if gi < FONT.len() { &FONT[gi] } else { &FONT[0] };
    let x0 = col * CW;
    let y0 = row * CH;
    if x0 + CW > W || y0 + CH > H {
        return;
    }
    let base = BASE as *mut u32;
    let fg = FG;
    let b = bg();
    for (ry, bits) in g.iter().enumerate() {
        let line = (y0 + ry) * STRIDE + x0;
        for rx in 0..CW {
            let on = bits & (0x80 >> rx) != 0;
            base.add(line + rx).write_volatile(if on { fg } else { b });
        }
    }
}

unsafe fn newline() {
    COL = 0;
    ROW += 1;
    if ROW >= rows() {
        // Simple page flip when the screen fills (cheap; scrolling is future).
        clear();
    }
}

pub fn putc(c: char) {
    unsafe {
        if !READY {
            return;
        }
        match c {
            '\n' => newline(),
            '\r' => COL = 0,
            '\u{8}' | '\u{7f}' => {
                if COL > 0 {
                    COL -= 1;
                    glyph(COL, ROW, b' ');
                }
            }
            ch if (ch as u32) >= 0x20 && (ch as u32) < 0x7f => {
                if COL >= cols() {
                    newline();
                }
                glyph(COL, ROW, ch as u8);
                COL += 1;
            }
            _ => {}
        }
    }
}

pub fn puts(s: &str) {
    for c in s.chars() {
        putc(c);
    }
}
