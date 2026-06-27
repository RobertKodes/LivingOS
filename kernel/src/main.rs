//! LivingOS — the Living Kernel.
//!
//! A bootable, `no_std` UEFI operating-system kernel where **AI agents are
//! first-class, kernel-managed resources**. This binary IS the OS image: UEFI
//! firmware loads `livingos.efi` and hands control to [`main`]. There is no
//! Windows, no Linux beneath it.
//!
//! Boot brings up the native agent subsystem, runs a self-test of the
//! capability gate, then drops into the **Living Shell** — an interactive,
//! on-OS console (keyboard or serial) where you state goals and watch the agent
//! society get scheduled to pursue them. Living Memory persists to the EFI
//! System Partition across reboots.

#![no_main]
#![no_std]
#![allow(dead_code)]

#[macro_use]
extern crate alloc;

/// The kernel console sink: the on-screen framebuffer text console **and** the
/// COM1 serial line. The whole session is thus visible both in the display and
/// over serial.
pub struct Out;
impl core::fmt::Write for Out {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::fbcon::puts(s);
        for b in s.bytes() {
            if b == b'\n' {
                crate::serial::putc(b'\r');
            }
            crate::serial::putc(b);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let _ = write!($crate::Out, $($arg)*);
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => {{ $crate::kprint!("\n"); }};
    ($($arg:tt)*) => {{ $crate::kprint!($($arg)*); $crate::kprint!("\n"); }};
}

mod audio;
mod bridge;
mod console;
mod fbcon;
mod font;
mod fs;
mod gop;
mod idt;
mod memgraph;
mod mm;
mod net;
mod nn;
mod nn_weights;
mod planner;
mod plugins;
mod sched;
mod selfhost;
mod serial;
mod shell;
mod society;
mod task;

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    serial::init();
    serial::init2(); // COM2: kernel<->host model bridge
    fbcon::init(); // on-screen framebuffer text console

    // Paint the GPU framebuffer splash, hold it, then clear to the console.
    if gop::splash().is_some() {
        boot::stall(1_500_000);
    }
    fbcon::clear();
    banner();
    let mut sh = shell::Shell::new();
    sh.boot_selftest();
    sh.run();
}

fn banner() {
    kprintln!();
    kprintln!("  LivingOS  ::  the Living Kernel");
    kprintln!("  an AI-native OS where agents are first-class kernel resources");
    kprintln!("--------------------------------------------------------------------");
    kprintln!("[boot] UEFI firmware handoff ............ ok");
    kprintln!("[boot] COM1 serial console ............. ok");
    kprintln!("[boot] global allocator ................ ok");
    kprintln!("[boot] agent subsystem ................. ok");
    kprintln!("[boot] living shell .................... ok");
}

/// Format a reputation float without libm (one decimal place).
pub(crate) fn rep(r: f32) -> alloc::string::String {
    let scaled = (r * 10.0 + 0.5) as i32;
    let whole = scaled / 10;
    let frac = scaled % 10;
    format!("{}.{}", whole, frac)
}
