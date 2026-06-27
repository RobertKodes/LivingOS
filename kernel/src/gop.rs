//! GPU framebuffer access via the UEFI Graphics Output Protocol (GOP).
//!
//! This is the seed of the LivingOS visual command center. It locates the
//! graphics device, reports the mode, and paints a boot splash directly into
//! the framebuffer using hardware block-transfer (Blt) fills — format-agnostic,
//! no per-pixel math. Everything is best-effort: on firmware without a GOP
//! (e.g. a pure serial console) the kernel simply skips graphics.

use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};

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
