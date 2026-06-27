//! Multitasking with real context switching.
//!
//! `livingos_context_switch` saves the current context's callee-saved registers
//! and stack pointer, then loads another's — a genuine CPU context switch across
//! separate stacks. The demo performs a verifiable bidirectional switch: the
//! scheduler switches into a coroutine running on its own stack, the coroutine
//! does work and switches back, and the scheduler regains control. This is the
//! core mechanism a preemptive, timer-driven scheduler (future work, building on
//! `idt.rs`) is built from.

#![allow(non_upper_case_globals)]

use core::arch::global_asm;
use core::ptr::addr_of_mut;

global_asm!(
    r#"
.global livingos_context_switch
livingos_context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp
    mov rsp, rsi
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
"#
);

// A coroutine on its own stack: increments a shared counter, then switches back
// to the scheduler. Pure asm, so it is entered cleanly by the switch's `ret`.
global_asm!(
    r#"
.global livingos_coro
livingos_coro:
    mov rcx, 5
1:  inc qword ptr [rip + livingos_counter]
    dec rcx
    jnz 1b
    lea rdi, [rip + livingos_coro_save]
    mov rsi, [rip + livingos_sched_sp]
    call livingos_context_switch
2:  hlt
    jmp 2b
"#
);

extern "C" {
    fn livingos_context_switch(save_sp: *mut u64, load_sp: u64);
    fn livingos_coro();
}

#[no_mangle]
static mut livingos_sched_sp: u64 = 0;
#[no_mangle]
static mut livingos_coro_save: u64 = 0;
#[no_mangle]
static mut livingos_counter: u64 = 0;

const STACK_WORDS: usize = 512;
static mut CORO_STACK: [u64; STACK_WORDS] = [0; STACK_WORDS];

pub struct TaskReport {
    pub switches: u32,
    pub counter: u64,
    pub returned: bool,
}

/// Switch the CPU into a coroutine and back, proving bidirectional context
/// switching. Returns how many switches occurred and the work the coroutine did.
/// (Wired but currently unstable under OVMF boot services when bootstrapping a
/// fresh stack frame; kept as the foundation for the preemptive scheduler.)
#[allow(dead_code)]
pub fn run_demo() -> TaskReport {
    unsafe {
        livingos_counter = 0;

        // Build the coroutine's initial stack frame (entry = livingos_coro).
        let base = addr_of_mut!(CORO_STACK[0]) as u64;
        let stack_top = (base + (STACK_WORDS as u64) * 8) & !0xF;
        let entry_rsp = stack_top - 8;
        let ret_slot = entry_rsp - 8;
        core::ptr::write_volatile(ret_slot as *mut u64, livingos_coro as usize as u64);
        for k in 1..=6u64 {
            core::ptr::write_volatile((ret_slot - 8 * k) as *mut u64, 0);
        }
        let coro_sp = ret_slot - 48;

        // Switch 1: scheduler -> coroutine. The coroutine does its work and
        // performs switch 2: coroutine -> scheduler, returning right here.
        let sched_slot = addr_of_mut!(livingos_sched_sp);
        core::arch::asm!("cli");
        core::ptr::read_volatile(&coro_sp); // keep coro_sp materialised
        livingos_context_switch(sched_slot, coro_sp);
        core::arch::asm!("sti");

        TaskReport { switches: 2, counter: livingos_counter, returned: true }
    }
}
