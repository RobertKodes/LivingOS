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

/// Print to BOTH the UEFI console and the COM1 serial line (no newline).
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        uefi::print!($($arg)*);
        { use core::fmt::Write as _; let _ = write!($crate::serial::Serial, $($arg)*); }
    }};
}

/// Print a line to BOTH the UEFI console and the COM1 serial line.
#[macro_export]
macro_rules! kprintln {
    () => {{
        uefi::println!();
        { use core::fmt::Write as _; let _ = writeln!($crate::serial::Serial); }
    }};
    ($($arg:tt)*) => {{
        uefi::println!($($arg)*);
        { use core::fmt::Write as _; let _ = writeln!($crate::serial::Serial, $($arg)*); }
    }};
}

mod audio;
mod bridge;
mod console;
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

    // Paint the GPU framebuffer splash (best-effort), then hold it briefly.
    match gop::splash() {
        Some((w, h)) => {
            kprintln!("[boot] GPU framebuffer ................. ok ({}x{})", w, h);
            boot::stall(1_500_000);
        }
        None => kprintln!("[boot] GPU framebuffer ................. none (serial console)"),
    }

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
