//! LivingOS — the Living Kernel.
//!
//! A bootable, `no_std` UEFI operating-system kernel where **AI agents are
//! first-class, kernel-managed resources**. This binary IS the OS image: UEFI
//! firmware loads `livingos.efi` and hands control to [`main`]. There is no
//! Windows, no Linux beneath it.
//!
//! At boot the kernel brings up its native agent subsystem, spawns the agent
//! society as kernel objects, and demonstrates the core machinery: a priority
//! scheduler over agents, a capability gate that every privileged action must
//! pass, a transparent audit trail, and reputation that feeds the Evolution
//! Engine. The language models that make agents *intelligent* live in user
//! space (see the host `crates/` runtime); the kernel owns the agents.

#![no_main]
#![no_std]
// The full capability/state model is intentionally complete; not every variant
// is exercised by the boot demo.
#![allow(dead_code)]

#[macro_use]
extern crate alloc;

mod sched;
mod serial;
mod society;

use alloc::string::String;
use sched::{Capability, Kernel, Scheduler};
use uefi::prelude::*;

/// Print a line to BOTH the UEFI graphics console and the COM1 serial line, so
/// the boot is visible on screen and capturable headlessly.
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

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    serial::init();
    boot();
    // The kernel never returns to firmware; idle forever.
    loop {
        boot::stall(10_000_000);
    }
}

fn rule() {
    kprintln!("--------------------------------------------------------------------");
}

fn boot() {
    kprintln!();
    kprintln!("  LivingOS  ::  the Living Kernel");
    kprintln!("  an AI-native OS where agents are first-class kernel resources");
    rule();

    // --- bring-up -----------------------------------------------------------
    kprintln!("[boot] UEFI firmware handoff ............ ok");
    kprintln!("[boot] COM1 serial console ............. ok");
    kprintln!("[boot] global allocator ................ ok");
    kprintln!("[boot] agent subsystem ................. ok");

    let mut k = Kernel::new();
    let mut sched = Scheduler::new();

    // --- spawn the society as kernel objects --------------------------------
    for spec in society::society() {
        let id = k.spawn(spec.name, spec.caps, spec.priority);
        kprintln!("[init] spawned agent #{} {:<10} ({})", id, spec.name, spec.blurb);
    }
    rule();

    // --- the agent table (like `ps`, but for agents) ------------------------
    kprintln!("AGENTS");
    kprintln!("  {:<3} {:<10} {:<8} {:<4} {}", "ID", "ROLE", "STATE", "REP", "CAPABILITIES");
    for a in k.agents() {
        kprintln!(
            "  {:<3} {:<10} {:<8} {:<4} {}",
            a.id,
            a.name,
            a.state.label(),
            rep(a.reputation),
            a.caps_label()
        );
    }
    rule();

    // --- the capability gate, demonstrated ----------------------------------
    kprintln!("CAPABILITY GATE");
    let eyes = k.find("Eyes").unwrap();
    let coder = k.find("Coder").unwrap();
    demo_gate(&mut k, eyes, Capability::ScreenCapture, "Eyes wants to capture the desktop");
    demo_gate(&mut k, coder, Capability::ScreenCapture, "Coder wants to capture the desktop");
    demo_gate(&mut k, coder, Capability::Compiler, "Coder wants the compiler");
    rule();

    // --- run a goal through the scheduler -----------------------------------
    kprintln!("GOAL  \"build a multiplayer game\"");
    sched.submit(String::from("research the genre and constraints"), "Researcher", 6);
    sched.submit(String::from("design the architecture"), "Architect", 7);
    sched.submit(String::from("review the security model"), "Security", 8);
    sched.submit(String::from("implement the core loop"), "Coder", 6);
    sched.submit(String::from("validate gameplay"), "Tester", 5);
    sched.submit(String::from("generate cover art"), "Designer", 5);
    kprintln!("[sched] {} tasks queued; dispatching by priority...", sched.pending());

    while let Some(task) = sched.next() {
        if let Some(id) = k.find(task.role) {
            k.set_state(id, sched::AgentState::Running);
            // Every task requires model inference — gated and logged.
            let needed = if task.role.eq_ignore_ascii_case("Designer") {
                Capability::ImageGen
            } else {
                Capability::ModelInference
            };
            let ok = k.authorize(id, needed, &task.title);
            kprintln!("  [run] {:<10} {}", task.role, task.title);
            k.record(id, ok, &task.title);
        }
    }
    rule();

    // --- transparent audit trail --------------------------------------------
    kprintln!("AUDIT TRAIL (every privileged action is explainable)");
    for line in k.audit.iter() {
        let mark = if line.allowed { "ok    " } else { "DENIED" };
        kprintln!("  {}  {:<10} {:<26} {}", mark, line.agent, line.action, line.detail);
    }
    rule();

    // --- final reputation (Evolution Engine) --------------------------------
    kprintln!("EVOLUTION  (reputation after this session)");
    for a in k.agents() {
        if a.tasks_done > 0 || a.tasks_failed > 0 {
            kprintln!(
                "  {:<10} rep {}   done {}  failed {}",
                a.name,
                rep(a.reputation),
                a.tasks_done,
                a.tasks_failed
            );
        }
    }
    rule();
    kprintln!("LivingOS is idle. Express a goal; the society will assemble.");
}

fn demo_gate(k: &mut Kernel, id: sched::AgentId, cap: Capability, reason: &str) {
    let allowed = k.authorize(id, cap, reason);
    let verdict = if allowed { "GRANTED" } else { "DENIED " };
    let name = k.agents().iter().find(|a| a.id == id).map(|a| a.name).unwrap_or("?");
    kprintln!("  [{}] {:<10} -> {}", verdict, name, cap.label());
}

/// Format a reputation float without needing libm (one decimal place).
fn rep(r: f32) -> alloc::string::String {
    let scaled = (r * 10.0 + 0.5) as i32; // round to tenths
    let whole = scaled / 10;
    let frac = scaled % 10;
    format!("{}.{}", whole, frac)
}
