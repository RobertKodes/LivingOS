//! Self-hosting — taking the machine from the firmware.
//!
//! This calls `ExitBootServices` (after which UEFI boot services, the firmware
//! console/keyboard/GOP/FS, and the heap are all gone) and then runs LivingOS on
//! **its own**: its own IDT, its own COM1 serial driver, direct framebuffer
//! writes (the GOP base grabbed before exit), and a PS/2 keyboard driver. From
//! this point the kernel owns the CPU — there is no firmware beneath it. The
//! transition is one-way, so this never returns.
//!
//! Post-exit there is no allocator, so everything here is stack/static only —
//! no `Vec`/`String`/`format!`, just raw bytes and the embedded font.

use core::arch::{asm, global_asm};
use core::ptr::addr_of_mut;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

use crate::font::FONT;

static mut FB_BASE: u64 = 0;
static mut FB_W: usize = 0;
static mut FB_H: usize = 0;
static mut FB_STRIDE: usize = 0;
static mut FB_BGR: bool = true;

// ---- port I/O --------------------------------------------------------------

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}
#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    v
}

// ---- serial (post-exit safe: raw port I/O, no firmware) --------------------

fn sputc(b: u8) {
    unsafe {
        while inb(0x3F8 + 5) & 0x20 == 0 {}
        outb(0x3F8, b);
    }
}
fn sputs(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            sputc(b'\r');
        }
        sputc(b);
    }
}
fn sput_hex(v: u64) {
    sputs("0x");
    let mut started = false;
    for i in (0..16).rev() {
        let nib = ((v >> (i * 4)) & 0xF) as u8;
        if nib != 0 || started || i == 0 {
            started = true;
            sputc(if nib < 10 { b'0' + nib } else { b'a' + nib - 10 });
        }
    }
}

// ---- framebuffer (direct writes to the GOP base) ---------------------------

fn grab_framebuffer() {
    if let Ok(h) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        if let Ok(mut gop) = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(h) {
            let info = gop.current_mode_info();
            let (w, hh) = info.resolution();
            let stride = info.stride();
            let bgr = !matches!(info.pixel_format(), PixelFormat::Rgb);
            let mut fb = gop.frame_buffer();
            unsafe {
                FB_BASE = fb.as_mut_ptr() as u64;
                FB_W = w;
                FB_H = hh;
                FB_STRIDE = stride;
                FB_BGR = bgr;
            }
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    unsafe {
        if FB_BGR {
            ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        } else {
            ((b as u32) << 16) | ((g as u32) << 8) | r as u32
        }
    }
}

unsafe fn fb_fill(x: usize, y: usize, w: usize, h: usize, color: u32) {
    if FB_BASE == 0 {
        return;
    }
    let base = FB_BASE as *mut u32;
    let yend = (y + h).min(FB_H);
    let xend = (x + w).min(FB_W);
    let mut yy = y;
    while yy < yend {
        let mut xx = x;
        while xx < xend {
            base.add(yy * FB_STRIDE + xx).write_volatile(color);
            xx += 1;
        }
        yy += 1;
    }
}

unsafe fn fb_text(mut x: usize, y: usize, scale: usize, s: &str, color: u32) {
    for ch in s.bytes() {
        let gi = (ch as usize).wrapping_sub(0x20);
        if gi < FONT.len() {
            let glyph = &FONT[gi];
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (0x80 >> col) != 0 {
                        fb_fill(x + col * scale, y + row * scale, scale, scale, color);
                    }
                }
            }
        }
        x += 8 * scale;
    }
}

// ---- our own IDT (minimal: every vector -> halt-safe iretq) ----------------

global_asm!(
    r#"
.global selfhost_default_isr
selfhost_default_isr:
    iretq
"#
);
extern "C" {
    fn selfhost_default_isr();
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Gate {
    low: u64,
    high: u64,
}
static mut IDT: [Gate; 256] = [Gate { low: 0, high: 0 }; 256];

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

unsafe fn load_idt() {
    let cs: u16;
    asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    let off = selfhost_default_isr as usize as u64;
    let low = (off & 0xFFFF)
        | ((cs as u64) << 16)
        | (0xEu64 << 40)
        | (1u64 << 47)
        | (((off >> 16) & 0xFFFF) << 48);
    let high = (off >> 32) & 0xFFFF_FFFF;
    for g in IDT.iter_mut() {
        g.low = low;
        g.high = high;
    }
    let idtr = Idtr { limit: (core::mem::size_of_val(&IDT) - 1) as u16, base: IDT.as_ptr() as u64 };
    asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack, preserves_flags));
}

fn busy_wait(units: u64) {
    let mut i = 0u64;
    while i < units {
        unsafe { asm!("pause", options(nomem, nostack, preserves_flags)) };
        i += 1;
    }
}

/// Take over the machine. Never returns.
pub fn enter() -> ! {
    grab_framebuffer();

    unsafe {
        // After this line the firmware is gone. Forget the returned map (no
        // dealloc — the allocator is dead now).
        let mm = uefi::boot::exit_boot_services(uefi::boot::MemoryType::LOADER_DATA);
        core::mem::forget(mm);
        asm!("cli");
        load_idt();
    }

    sputs("\n");
    sputs("================================================\n");
    sputs("  LivingOS :: SELF-HOSTED\n");
    sputs("  ExitBootServices done -- firmware released\n");
    sputs("  running on own IDT + COM1 + framebuffer + PS/2\n");
    sputs("  framebuffer base ");
    unsafe { sput_hex(FB_BASE) };
    sputs("\n================================================\n");

    unsafe {
        fb_fill(0, 0, FB_W, FB_H, rgb(8, 10, 16));
        fb_fill(0, 0, FB_W, 60, rgb(180, 40, 60));
        fb_text(16, 18, 2, "LivingOS  self-hosted", rgb(245, 245, 255));
        fb_text(16, 90, 1, "ExitBootServices complete -- no firmware beneath the kernel.", rgb(200, 205, 220));
        fb_text(16, 116, 1, "own IDT loaded; COM1, framebuffer and PS/2 drivers live.", rgb(200, 205, 220));
    }

    sputs("PS/2 keyboard driver live (poll 0x60/0x64). Idle loop; press keys.\n");

    // Self-hosted idle loop: poll the PS/2 keyboard and echo scancodes.
    let mut tick: u64 = 0;
    loop {
        unsafe {
            if inb(0x64) & 1 != 0 {
                let sc = inb(0x60);
                sputs("key scancode ");
                sput_hex(sc as u64);
                sputc(b'\n');
            }
        }
        busy_wait(200_000);
        tick += 1;
        if tick % 50 == 0 {
            sputc(b'.');
        }
    }
}
