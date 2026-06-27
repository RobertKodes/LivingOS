//! Persistence for Living Memory. Reads and writes a single file on the EFI
//! System Partition we booted from, using UEFI's Simple File System protocol —
//! so the memory graph survives across reboots. Best-effort: every operation
//! degrades to "no persistence" rather than failing the kernel.

use alloc::string::String;
use uefi::cstr16;
use uefi::fs::FileSystem;
use uefi::CStr16;

const MEM_FILE: &CStr16 = cstr16!("livingos.mem");
const PLUGINS_FILE: &CStr16 = cstr16!("plugins.cfg");

fn filesystem() -> Option<FileSystem> {
    let handle = uefi::boot::image_handle();
    let proto = uefi::boot::get_image_file_system(handle).ok()?;
    Some(FileSystem::new(proto))
}

fn read_file(name: &CStr16) -> Option<String> {
    let mut fs = filesystem()?;
    let bytes = fs.read(name).ok()?;
    String::from_utf8(bytes).ok()
}

fn write_file(name: &CStr16, blob: &str) -> bool {
    match filesystem() {
        Some(mut fs) => fs.write(name, blob.as_bytes()).is_ok(),
        None => false,
    }
}

/// Load the serialized memory blob, if present.
pub fn load() -> Option<String> {
    read_file(MEM_FILE)
}

/// Persist the serialized memory blob. Returns whether it was written.
pub fn save(blob: &str) -> bool {
    write_file(MEM_FILE, blob)
}

/// Load the plugin manifest, if present.
pub fn load_plugins() -> Option<String> {
    read_file(PLUGINS_FILE)
}

/// Write the plugin manifest (used to seed a default on first boot).
pub fn save_plugins(blob: &str) -> bool {
    write_file(PLUGINS_FILE, blob)
}
