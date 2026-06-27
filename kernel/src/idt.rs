//! Syscall bridge — a real interrupt-driven syscall boundary.
//!
//! Installs the kernel's own Interrupt Descriptor Table with a handler on vector
//! 0x80, then issues `int 0x80` from "user" code: the CPU traps into the handler
//! (`livingos_syscall_isr`), which dispatches by syscall number (in rax) to a
//! kernel service and returns the result in rax — exactly the mechanism a
//! user-space agent runtime would use to call kernel services. The dispatch is
//! pure assembly (no C-ABI/stack juggling in interrupt context), and we save and
//! restore the firmware IDTR around the demo so UEFI is undisturbed.

use core::arch::{asm, global_asm};

// Live kernel value the SYS_AGENTS syscall reads; updated before the demo.
#[no_mangle]
static mut livingos_agent_count: u64 = 0;

global_asm!(
    r#"
.global livingos_syscall_isr
livingos_syscall_isr:
    cmp rax, 1
    je 1f
    cmp rax, 2
    je 2f
    cmp rax, 3
    je 3f
    cmp rax, 4
    je 4f
    mov rax, -1            # unknown syscall
    iretq
1:  lea rax, [rdi + 1]     # SYS_INC
    iretq
2:  lea rax, [rdi + rdi]   # SYS_DOUBLE
    iretq
3:  movabs rax, 0x4C4956494E47   # SYS_VERSION -> "LIVING"
    iretq
4:  mov rax, [rip + livingos_agent_count]   # SYS_AGENTS
    iretq

.global livingos_default_isr
livingos_default_isr:
    iretq
"#
);

extern "C" {
    fn livingos_syscall_isr();
    fn livingos_default_isr();
}

/// A 64-bit IDT gate descriptor as two u64 halves, built with explicit bit math.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct IdtEntry {
    low: u64,
    high: u64,
}

impl IdtEntry {
    const fn zero() -> Self {
        IdtEntry { low: 0, high: 0 }
    }
    fn set(&mut self, handler: u64, sel: u16) {
        let off = handler;
        self.low = (off & 0xFFFF)
            | ((sel as u64) << 16)
            | (0xEu64 << 40) // type = 64-bit interrupt gate
            | (1u64 << 47) // present
            | (((off >> 16) & 0xFFFF) << 48);
        self.high = (off >> 32) & 0xFFFF_FFFF;
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

#[inline(never)]
fn do_syscall(num: u64, arg: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!("int 0x80", in("rax") num, in("rdi") arg, lateout("rax") ret);
    }
    ret
}

pub struct SyscallReport {
    pub calls: [(u64, u64, u64); 4],
    pub expected_base: u64,
    pub actual_base: u64,
}

/// Install our IDT, issue a few syscalls, restore the firmware IDT.
pub fn run_syscall_demo(agent_count: u64) -> SyscallReport {
    unsafe {
        core::ptr::addr_of_mut!(livingos_agent_count).write(agent_count);
    }

    let mut idt = [IdtEntry::zero(); 256];
    let cs: u16;
    unsafe {
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    }
    let def = livingos_default_isr as usize as u64;
    let sys = livingos_syscall_isr as usize as u64;
    for e in idt.iter_mut() {
        e.set(def, cs);
    }
    idt[0x80].set(sys, cs);

    let mut old = Idtr { limit: 0, base: 0 };
    let new = Idtr {
        limit: (core::mem::size_of_val(&idt) - 1) as u16,
        base: idt.as_ptr() as u64,
    };
    let expected_base = idt.as_ptr() as u64;
    let mut check = Idtr { limit: 0, base: 0 };
    let calls = [(1u64, 41u64), (2, 21), (3, 0), (4, 0)];
    let mut out = [(0u64, 0u64, 0u64); 4];
    unsafe {
        asm!("sidt [{}]", in(reg) &mut old, options(nostack, preserves_flags));
        asm!("lidt [{}]", in(reg) &new, options(readonly, nostack, preserves_flags));
        asm!("sidt [{}]", in(reg) &mut check, options(nostack, preserves_flags));
        for (i, &(n, a)) in calls.iter().enumerate() {
            out[i] = (n, a, do_syscall(n, a));
        }
        asm!("lidt [{}]", in(reg) &old, options(readonly, nostack, preserves_flags));
    }
    SyscallReport { calls: out, expected_base, actual_base: check.base }
}
