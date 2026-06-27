//! The kernel↔user-space model bridge.
//!
//! On bare metal the kernel cannot host the large local language models, so —
//! exactly as the PRD intends (models live in user space) — it talks to a model
//! service over a channel. Here that channel is COM2: the kernel writes a
//! request, a host-side daemon (`tools/model_bridge.py`) routes it to the local
//! models (Ollama / the Intelligence Router) and writes the answer back. This is
//! the seam that lets real local-model intelligence drive the on-OS agents,
//! replacing the in-kernel keyword planner behind the *same* pipeline.
//!
//! Wire protocol (line-oriented over COM2):
//!   kernel -> host:  "ASK\t<text>\n"
//!   host -> kernel:  "ANS\t<text>\n"

use alloc::string::{String, ToString};

fn send_line(prefix: &str, text: &str) {
    for b in prefix.bytes() {
        crate::serial::putc2(b);
    }
    for b in text.bytes() {
        if b == b'\n' || b == b'\r' {
            crate::serial::putc2(b' ');
        } else {
            crate::serial::putc2(b);
        }
    }
    crate::serial::putc2(b'\n');
}

/// Send a request to the model bridge and wait (with timeout) for the answer.
/// Returns None if no host daemon responds.
pub fn ask(kind: &str, text: &str) -> Option<String> {
    // Drain any stale bytes.
    while crate::serial::try_read_byte2().is_some() {}

    let mut tag = String::from(kind);
    tag.push('\t');
    send_line(&tag, text);

    let mut line = String::new();
    let mut idle: u64 = 0;
    loop {
        match crate::serial::try_read_byte2() {
            Some(b'\n') => break,
            Some(b'\r') => {}
            Some(b) => {
                line.push(b as char);
                idle = 0;
            }
            None => {
                idle += 1;
                // ~30 seconds of patience for the host model call (1 ms per poll;
                // a wall-clock budget, not a spin count, so it survives a fast CPU).
                if idle > 30000 {
                    return None;
                }
                uefi::boot::stall(1000);
            }
        }
    }

    if let Some(rest) = line.strip_prefix("ANS\t") {
        Some(rest.to_string())
    } else if line.is_empty() {
        None
    } else {
        Some(line)
    }
}
