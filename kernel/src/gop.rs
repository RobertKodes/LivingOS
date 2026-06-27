//! GPU framebuffer access via the UEFI Graphics Output Protocol (GOP).
//!
//! This is the seed of the LivingOS visual command center. It locates the
//! graphics device, reports the mode, and paints a boot splash directly into
//! the framebuffer using hardware block-transfer (Blt) fills — format-agnostic,
//! no per-pixel math. Everything is best-effort: on firmware without a GOP
//! (e.g. a pure serial console) the kernel simply skips graphics.

use alloc::vec;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

use crate::font;

type Rgb = (u8, u8, u8);

fn fill(gop: &mut GraphicsOutput, x: usize, y: usize, w: usize, h: usize, r: u8, g: u8, b: u8) {
    let (rw, rh) = gop.current_mode_info().resolution();
    if x >= rw || y >= rh {
        return;
    }
    let w = w.min(rw - x);
    let h = h.min(rh - y);
    let _ = gop.blt(BltOp::VideoFill {
        color: BltPixel::new(r, g, b),
        dest: (x, y),
        dims: (w, h),
    });
}

/// Draw a string into the framebuffer using the 8x16 bitmap font. Builds one
/// Blt buffer for the whole string and blits it once.
fn draw_text(gop: &mut GraphicsOutput, x: usize, y: usize, scale: usize, s: &str, fg: Rgb, bg: Rgb) {
    let (rw, rh) = gop.current_mode_info().resolution();
    let cw = 8 * scale;
    let ch = 16 * scale;
    let width = cw * s.len();
    let height = ch;
    if width == 0 || x >= rw || y >= rh || x + width > rw || y + height > rh {
        return;
    }
    let fgp = BltPixel::new(fg.0, fg.1, fg.2);
    let mut buf = vec![BltPixel::new(bg.0, bg.1, bg.2); width * height];
    for (i, chr) in s.chars().enumerate() {
        let gi = (chr as usize).wrapping_sub(0x20);
        if gi >= font::FONT.len() {
            continue;
        }
        let glyph = &font::FONT[gi];
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = i * cw + col * scale + dx;
                            let py = row * scale + dy;
                            buf[py * width + px] = fgp;
                        }
                    }
                }
            }
        }
    }
    let _ = gop.blt(BltOp::BufferToVideo {
        buffer: &buf,
        src: BltRegion::Full,
        dest: (x, y),
        dims: (width, height),
    });
}

/// Render the visual command center: the agent society as a grid of cards with
/// reputation bars, plus a header and footer. A one-shot snapshot painted to the
/// GPU framebuffer (the live, always-on compositor is future work).
pub fn render_dashboard(agents: &[(&str, f32)], footer: &str) -> bool {
    let handle = match uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(h) => h,
        Err(_) => return false,
    };
    let mut gop = match uefi::boot::open_protocol_exclusive::<GraphicsOutput>(handle) {
        Ok(g) => g,
        Err(_) => return false,
    };
    let (w, h) = gop.current_mode_info().resolution();

    fill(&mut gop, 0, 0, w, h, 10, 12, 18);
    fill(&mut gop, 0, 0, w, 44, 24, 110, 200);
    fill(&mut gop, 0, 44, w, 3, 90, 200, 255);
    draw_text(&mut gop, 14, 12, 2, "LivingOS  command center", (240, 244, 255), (24, 110, 200));

    let palette: [Rgb; 9] = [
        (90, 200, 255), (120, 220, 180), (200, 200, 120), (220, 150, 120),
        (200, 120, 200), (150, 160, 240), (120, 220, 220), (240, 200, 120), (180, 180, 200),
    ];

    let cols = 3usize;
    let margin = 36usize;
    let top = 70usize;
    let card_w = (w.saturating_sub(margin * (cols + 1))) / cols;
    let card_h = 92usize;
    let v_gap = 22usize;

    for (idx, (name, rep)) in agents.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let x = margin + col * (card_w + margin);
        let y = top + row * (card_h + v_gap);
        let c = palette[idx % palette.len()];

        fill(&mut gop, x, y, card_w, card_h, 22, 28, 40);
        fill(&mut gop, x, y, 6, card_h, c.0, c.1, c.2); // accent stripe
        draw_text(&mut gop, x + 18, y + 14, 2, name, (235, 238, 245), (22, 28, 40));

        // reputation bar (rep is 0..5)
        let bar_x = x + 18;
        let bar_y = y + 52;
        let bar_w = card_w.saturating_sub(36);
        fill(&mut gop, bar_x, bar_y, bar_w, 16, 40, 46, 60);
        let frac = (rep / 5.0).clamp(0.0, 1.0);
        let filled = (bar_w as f32 * frac) as usize;
        fill(&mut gop, bar_x, bar_y, filled, 16, c.0, c.1, c.2);
    }

    fill(&mut gop, 0, h.saturating_sub(30), w, 30, 18, 22, 32);
    draw_text(&mut gop, 14, h.saturating_sub(26), 1, footer, (150, 160, 180), (18, 22, 32));
    true
}

/// Query the current framebuffer resolution, if a GOP is present.
pub fn resolution() -> Option<(usize, usize)> {
    let handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
    let gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(handle).ok()?;
    Some(gop.current_mode_info().resolution())
}

/// Paint the LivingOS boot splash. Returns the detected resolution, if any.
pub fn splash() -> Option<(usize, usize)> {
    let handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(handle).ok()?;

    let (w, h) = gop.current_mode_info().resolution();

    // Background.
    fill(&mut gop, 0, 0, w, h, 10, 12, 18);
    // Title band.
    fill(&mut gop, 0, 0, w, 64, 24, 110, 200);
    // Accent underline.
    fill(&mut gop, 0, 64, w, 4, 90, 200, 255);

    // Centred emblem: nested squares (the "living" core).
    let cx = w / 2;
    let cy = h / 2;
    let sizes: [(usize, (u8, u8, u8)); 4] = [
        (220, (24, 110, 200)),
        (160, (40, 160, 220)),
        (100, (90, 200, 255)),
        (44, (240, 240, 255)),
    ];
    for (s, (r, g, b)) in sizes {
        if s / 2 <= cx && s / 2 <= cy {
            fill(&mut gop, cx - s / 2, cy - s / 2, s, s, r, g, b);
        }
    }

    // A row of nine agent indicators near the bottom.
    let n = 9;
    let dot = 26usize;
    let gap = 18usize;
    let total = n * dot + (n - 1) * gap;
    let mut x = cx.saturating_sub(total / 2);
    let y = h.saturating_sub(90);
    let palette: [(u8, u8, u8); 9] = [
        (90, 200, 255), (120, 220, 180), (200, 200, 120), (220, 150, 120),
        (200, 120, 200), (150, 160, 240), (120, 220, 220), (240, 200, 120), (180, 180, 200),
    ];
    for (r, g, b) in palette {
        fill(&mut gop, x, y, dot, dot, r, g, b);
        x += dot + gap;
    }

    // Footer status bar.
    fill(&mut gop, 0, h.saturating_sub(28), w, 28, 18, 22, 32);

    Some((w, h))
}
