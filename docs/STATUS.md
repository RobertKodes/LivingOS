# LivingOS — status & honest roadmap

LivingOS is an experimental, early-stage OS. This document is a candid account
of what actually works today versus what the vision still needs. It is kept
honest on purpose: the project is more interesting as a real, bounded thing than
as an overclaim.

## What works today (verified booting in QEMU, UEFI/OVMF)

**The Living Kernel (`kernel/`, `no_std` UEFI)**
- Agents are first-class kernel objects (Agent Control Blocks): identity,
  capability set, lifecycle state, priority, reputation.
- **Capability gate** — every privileged action is checked and may be *denied*
  (e.g. only `Eyes` may `screen_capture`; `Coder` is refused). Verified live.
- **Priority scheduler** over agents (Native Agent Scheduler).
- **Transparent audit trail** — every authorization and outcome is logged.
- **Reputation** moves with task outcomes (Evolution Engine signal).
- **Inter-agent message bus (IPC)** — kernel-routed, logged; the society
  collaborates during a goal (`msgs`).
- **On-device planner** — decomposes a goal into role-assigned tasks by keyword.
- **Living Memory** — an in-kernel graph, **persisted to a real FAT disk image**
  across reboots (`tools/mkimage`; verified 8 nodes / 7 edges restored intact).
- **GPU framebuffer** — boot splash + a **visual command center** (`dash`) drawn
  with an embedded bitmap font (`docs/splash.png`, `docs/command-center.png`).
- **Interactive Living Shell** — a REPL over keyboard *or* serial.
- **Syscall bridge** — installs the kernel's own IDT; `int 0x80` from user code
  traps into a kernel handler that dispatches by syscall number (`syscall`).
- **Memory management / paging** — reads CR3, sums RAM from the UEFI memory map,
  allocates physical frames, and installs a live virtual→physical mapping by
  editing the page tables (`vm`).
- **On-metal neural-net inference** — a real (tiny) char-level MLP runs *in the
  kernel* with embedded trained weights (`gen`; `tools/train_nn.py`).
- **Audio** — drives the PC speaker (`beep`).
- **Plugin system** — data-driven agents loaded from a `plugins.cfg` manifest on
  the ESP, under the same capability gate (`plugins`).
- **Networking** — drives a real NIC via the UEFI Simple Network Protocol: reads
  the MAC, brings the link up, and performs an ARP exchange (`net`).
- **Context-switch primitive** — `livingos_context_switch` saves/restores a
  context's callee-saved registers + stack pointer (`task.rs`).

**User-space layer (`crates/`, host)**
- Agent Society, Intelligence Router (Ollama + local image server), Living
  Memory, the Eyes perception agent, the Designer image agent, and the `living`
  CLI. Builds and unit-tested. Runs on a host today (not yet on the kernel).

## Partially done
- **Visual command center** — `dash` renders a real framebuffer dashboard, but
  it is a one-shot snapshot; a live, always-on compositor is future.
- **Multitasking** — the context-switch primitive is implemented and the switch
  mechanism works in isolation, but the live coroutine round-trip is unstable
  under OVMF boot services when bootstrapping a fresh stack frame. Preemptive,
  timer-driven scheduling (on the IDT) is the next step.
- **Networking** — real NIC access + ARP works; a full TCP/IP stack does not yet
  exist, and QEMU's SLIRP NAT does not reliably ARP-reply (real hardware does).
- **Intelligence** — on the kernel, planning is a keyword heuristic and the
  in-kernel model is tiny; the real local specialist models live in the
  user-space router and don't yet drive the on-OS agents (needs the bridge).

## Not yet implemented (the genuinely large items)
- **Kernel ↔ user-space model bridge** — so the local specialist models drive
  the on-OS society, replacing the keyword planner behind the same pipeline.
- **Self-hosting** — `ExitBootServices` + own GDT/IDT/timer and native drivers
  (disk, NIC, PS/2) for the post-firmware world. Today the kernel relies on UEFI
  boot services (a legitimate prototype choice). Note: doing this means
  re-implementing the console, keyboard, GOP, and FS we currently get for free.
- **Preemptive multitasking** — live, timer-driven context switching.
- **Full TCP/IP stack** and the active `Internet` capability.
- **On-metal *large* model inference** (GPU-accelerated) and **voice I/O**
  (STT/TTS with the small local models).

## Next milestones (suggested order)
1. The kernel↔model bridge (biggest leverage for "intelligence on the OS").
2. Stabilise live preemptive multitasking on the IDT timer.
3. A minimal UDP/TCP layer over the working SNP/ARP base.
4. A live framebuffer command-center compositor.
