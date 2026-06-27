//! Memory management — physical frame allocation and 4-level paging.
//!
//! UEFI hands us an identity-mapped address space with the page tables it built.
//! This module reads CR3, walks/edits those tables to install a *new* virtual→
//! physical mapping (allocating intermediate page-table frames as needed), and
//! proves it: a value written through the freshly mapped virtual address is
//! readable through the frame's identity-mapped physical address. It also
//! summarises usable RAM from the UEFI memory map. Real VMM primitives — a full
//! self-owned VM subsystem (post-ExitBootServices) builds on exactly these.

use core::arch::asm;
use uefi::boot::{self, AllocateType};
use uefi::mem::memory_map::{MemoryMap, MemoryType};

const PRESENT: u64 = 1;
const WRITE: u64 = 2;
const FRAME_MASK: u64 = 0x000F_FFFF_FFFF_F000;

fn read_cr3() -> u64 {
    let v: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags));
    }
    v & FRAME_MASK
}

/// OVMF maps its page tables read-only and sets CR0.WP, so supervisor writes to
/// them fault. Clear WP for the duration of a table edit, then restore.
unsafe fn clear_wp() -> u64 {
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    asm!("mov cr0, {}", in(reg) cr0 & !0x0001_0000u64, options(nomem, nostack, preserves_flags));
    cr0
}

unsafe fn restore_cr0(cr0: u64) {
    asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
}

/// Allocate one zeroed 4 KiB physical frame (identity-mapped, so the returned
/// address is directly usable as a pointer).
fn alloc_frame() -> Option<u64> {
    let ptr = boot::allocate_pages(AllocateType::AnyPages, boot::MemoryType::LOADER_DATA, 1).ok()?;
    let addr = ptr.as_ptr() as u64;
    unsafe {
        core::ptr::write_bytes(addr as *mut u8, 0, 4096);
    }
    Some(addr)
}

unsafe fn ensure(table: u64, idx: usize) -> Option<u64> {
    let entry_ptr = (table as *mut u64).add(idx);
    let entry = entry_ptr.read_volatile();
    if entry & PRESENT == 0 {
        let frame = alloc_frame()?;
        entry_ptr.write_volatile(frame | PRESENT | WRITE);
        Some(frame)
    } else {
        Some(entry & FRAME_MASK)
    }
}

/// Map `virt` → `phys` in the active page tables.
unsafe fn map(virt: u64, phys: u64) -> Option<()> {
    let pml4 = read_cr3();
    let i4 = ((virt >> 39) & 0x1FF) as usize;
    let i3 = ((virt >> 30) & 0x1FF) as usize;
    let i2 = ((virt >> 21) & 0x1FF) as usize;
    let i1 = ((virt >> 12) & 0x1FF) as usize;
    let pdpt = ensure(pml4, i4)?;
    let pd = ensure(pdpt, i3)?;
    let pt = ensure(pd, i2)?;
    (pt as *mut u64).add(i1).write_volatile((phys & FRAME_MASK) | PRESENT | WRITE);
    asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    Some(())
}

fn total_conventional_mib() -> u64 {
    match boot::memory_map(boot::MemoryType::LOADER_DATA) {
        Ok(mm) => {
            let mut pages = 0u64;
            for d in mm.entries() {
                if d.ty == MemoryType::CONVENTIONAL {
                    pages += d.page_count;
                }
            }
            pages * 4096 / 1024 / 1024
        }
        Err(_) => 0,
    }
}

pub struct PagingReport {
    pub ram_mib: u64,
    pub cr3: u64,
    pub frame: u64,
    pub virt: u64,
    pub wrote: u64,
    pub via_virt: u64,
    pub via_phys: u64,
    pub ok: bool,
}

/// Allocate a frame, map a fresh virtual page to it, and verify the mapping.
pub fn run_paging_demo() -> PagingReport {
    let ram_mib = total_conventional_mib();
    let cr3 = read_cr3();
    let virt = 0x0000_4000_0000_0000u64; // a high, unused virtual address
    let pattern = 0xCAFEBABE_DEADBEEFu64;

    let frame = match alloc_frame() {
        Some(f) => f,
        None => {
            return PagingReport { ram_mib, cr3, frame: 0, virt, wrote: pattern, via_virt: 0, via_phys: 0, ok: false }
        }
    };

    let (via_virt, via_phys, ok) = unsafe {
        let saved = clear_wp(); // allow writes to OVMF's read-only page tables
        let mapped = map(virt, frame).is_some();
        restore_cr0(saved);
        if mapped {
            (virt as *mut u64).write_volatile(pattern); // write through the NEW virtual mapping
            let v = (virt as *mut u64).read_volatile();
            let p = (frame as *mut u64).read_volatile(); // read the same bytes via the physical frame
            (v, p, v == pattern && p == pattern)
        } else {
            (0, 0, false)
        }
    };

    PagingReport { ram_mib, cr3, frame, virt, wrote: pattern, via_virt, via_phys, ok }
}
