//! Plugin system — data-driven OS extensions.
//!
//! LivingOS is meant to be composable: new agents can be added without
//! recompiling the kernel. A plugin manifest (`plugins.cfg`) on the EFI System
//! Partition declares extra agents to spawn at boot, each with its own scoped
//! capabilities. The same capability gate applies to plugin agents — they get
//! no more than the manifest grants. On first boot a default manifest is seeded
//! so the mechanism is visible and user-editable.
//!
//! Manifest format, one agent per line:
//!   name, priority, cap1|cap2|...
//! Lines starting with '#' are comments.

use crate::sched::Capability;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;

pub struct PluginSpec {
    pub name: &'static str,
    pub priority: u8,
    pub caps: Vec<Capability>,
}

const DEFAULT_MANIFEST: &str = "\
# LivingOS plugins — one agent per line:  name, priority, cap1|cap2|...
# Edit this file on the ESP to add your own agents; the capability gate applies.
Translator, 5, model_inference|memory
Scheduler, 6, model_inference|memory|terminal
Sentinel, 8, model_inference|internet
";

fn parse(manifest: &str) -> Vec<PluginSpec> {
    let mut out = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(',');
        let name = match parts.next() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let priority = parts.next().and_then(|p| p.trim().parse::<u8>().ok()).unwrap_or(5);
        let mut caps = Vec::new();
        if let Some(capstr) = parts.next() {
            for c in capstr.split('|') {
                if let Some(cap) = Capability::parse(c) {
                    caps.push(cap);
                }
            }
        }
        // Leak the name to obtain the &'static str the kernel uses for agents.
        let name: &'static str = Box::leak(name.into_boxed_str());
        out.push(PluginSpec { name, priority, caps });
    }
    out
}

/// Load plugin specs from the ESP manifest, seeding a default on first boot.
pub fn load() -> Vec<PluginSpec> {
    let manifest = match crate::fs::load_plugins() {
        Some(m) => m,
        None => {
            crate::fs::save_plugins(DEFAULT_MANIFEST);
            DEFAULT_MANIFEST.to_string()
        }
    };
    parse(&manifest)
}
