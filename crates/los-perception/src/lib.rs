//! los-perception — the Eyes of LivingOS.
//!
//! Captures the desktop (all monitors, or one) and returns PNG bytes ready to
//! hand to the Intelligence Router's vision specialist. The Perception agent in
//! the runtime can only call this after the kernel authorizes its
//! `ScreenCapture` capability — so "the OS can see the screen" is a privilege
//! that is granted, gated, and logged, never ambient.

use std::io::Cursor;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Frame {
    pub monitor: String,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header()?;
        writer.write_image_data(rgba)?;
    }
    Ok(out.into_inner())
}

/// Capture every monitor as a separate PNG frame.
pub fn capture_all() -> Result<Vec<Frame>> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("enumerate monitors: {e}"))?;
    if monitors.is_empty() {
        return Err("no monitors found".into());
    }
    let mut frames = Vec::new();
    for m in monitors {
        let name = m.name().to_string();
        let img = m.capture_image().map_err(|e| format!("capture {name}: {e}"))?;
        let (w, h) = (img.width(), img.height());
        let png = encode_png(w, h, img.as_raw())?;
        frames.push(Frame { monitor: name, width: w, height: h, png });
    }
    Ok(frames)
}

/// Capture the primary monitor (the one at origin, falling back to the first).
pub fn capture_primary() -> Result<Frame> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("enumerate monitors: {e}"))?;
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary())
        .or_else(|| xcap::Monitor::all().ok().and_then(|mut v| v.drain(..).next()))
        .ok_or("no monitors found")?;
    let name = primary.name().to_string();
    let img = primary.capture_image().map_err(|e| format!("capture {name}: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let png = encode_png(w, h, img.as_raw())?;
    Ok(Frame { monitor: name, width: w, height: h, png })
}
