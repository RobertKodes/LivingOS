//! Audio out — the PC speaker (PIT channel 2 + port 0x61).
//!
//! This is real hardware sound: we program the 8253/8254 timer's channel 2 to a
//! frequency and gate it to the speaker. It is intentionally minimal — a beep,
//! not speech. Full voice I/O (STT/TTS with the small local models) needs the
//! user-space model bridge; this proves the kernel can drive the audio hardware.

use core::arch::asm;

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

fn speaker_off() {
    unsafe {
        let v = inb(0x61) & 0xFC;
        outb(0x61, v);
    }
}

/// Play a tone of `freq` Hz for `ms` milliseconds (freq 0 = silence/rest).
pub fn tone(freq: u32, ms: u64) {
    if freq == 0 {
        speaker_off();
        uefi::boot::stall((ms * 1000) as usize);
        return;
    }
    let divisor = 1_193_180u32 / freq;
    unsafe {
        outb(0x43, 0xB6); // channel 2, lobyte/hibyte, square wave
        outb(0x42, (divisor & 0xFF) as u8);
        outb(0x42, ((divisor >> 8) & 0xFF) as u8);
        let v = inb(0x61);
        if v & 3 != 3 {
            outb(0x61, v | 3); // gate + enable speaker
        }
    }
    uefi::boot::stall((ms * 1000) as usize);
    speaker_off();
}

/// A short startup chime.
pub fn chime() {
    tone(660, 120);
    tone(880, 120);
    tone(990, 160);
    speaker_off();
}
