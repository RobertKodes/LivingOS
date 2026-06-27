//! Persistence for Living Memory. Reads and writes a single file on the EFI
//! System Partition we booted from, using UEFI's Simple File System protocol —
//! so the memory graph survives across reboots. Best-effort: every operation
//! degrades to "no persistence" rather than failing the kernel.

use alloc::string::String;
use uefi::cstr16;
use uefi::fs::FileSystem;
use uefi::CStr16;

const FILE: &CStr16 = cstr16!("livingos.mem");

fn filesystem() -> Option<FileSystem> {
    let handle = uefi::boot::image_handle();
    let proto = uefi::boot::get_image_file_system(handle).ok()?;
    Some(FileSystem::new(proto))
}

/// Load the serialized memory blob, if present.
pub fn load() -> Option<String> {
    let mut fs = filesystem()?;
    let bytes = fs.read(FILE).ok()?;
    String::from_utf8(bytes).ok()
}

/// Persist the serialized memory blob. Returns whether it was written.
pub fn save(blob: &str) -> bool {
    match filesystem() {
        Some(mut fs) => fs.write(FILE, blob.as_bytes()).is_ok(),
        None => false,
    }
}
