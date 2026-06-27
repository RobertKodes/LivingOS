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
  collaborates during a goal.
- **On-device planner** — decomposes a goal into role-assigned tasks by keyword
  (a deterministic stand-in for the user-space model router).
- **Living Memory** — an in-kernel graph (goals/tasks/answers + edges).
- **Persistence** — the memory graph is saved to / restored from the EFI System
  Partition and survives reboots, with full fidelity over a real FAT disk image
  built by `tools/mkimage` (verified across sessions: 8 nodes / 7 edges restored
  intact).
- **GPU framebuffer** — a boot splash via the UEFI Graphics Output Protocol
  (1280×800 in QEMU; `docs/splash.png`), plus a **visual command center**
  (`dash`) drawn with an embedded bitmap font (`docs/command-center.png`).
- **Interactive Living Shell** — a REPL over keyboard *or* serial:
  `help, ps, goal, mem, recall, log, msgs, sys, about, clear, shutdown`.

**User-space layer (`crates/`, host)**
- Agent Society, Intelligence Router (Ollama + local image server), Living
  Memory, the Eyes perception agent, the Designer image agent, and the `living`
  CLI. Builds and unit-tested. Runs on a host today (not yet on the kernel).

## Partially done
- **Visual command center** — the `dash` command renders a real framebuffer
  dashboard (agent cards, reputation bars, own bitmap font; see
  `docs/command-center.png`). It is a one-shot *snapshot*; a live, always-on
  compositor that updates while agents work is still future.
- **Intelligence** — on the kernel, planning is a keyword heuristic. The real
  local models exist in the user-space router but don't yet drive the on-OS
  agents (see the bridge below).

## Not yet implemented (the genuinely large items)
These are each substantial efforts; none are faked in the codebase.
- **Kernel ↔ user-space syscall bridge** — an ABI so a user-space agent runtime
  (the `crates/` layer) can call the kernel's agent/capability/IPC services.
  This is the seam that would let the local models drive the on-OS society.
- **Take over the machine** — `ExitBootServices`, own GDT/IDT, a physical frame
  allocator + paging. Today the kernel runs on UEFI boot services (which is a
  legitimate prototype choice, but not yet a self-hosting kernel). Note: doing
  this means re-implementing the console, keyboard, timer, GOP, and FS that we
  currently get from firmware.
- **Preemptive multitasking** — real task contexts and context switching; the
  scheduler is currently run-to-completion.
- **Drivers** — disk (AHCI/NVMe/virtio), NIC, native keyboard (PS/2) for the
  post-boot-services world.
- **Networking** — a TCP/IP stack (the `Internet` capability is declared but
  inert).
- **On-device model inference** — running the small local models on the OS
  itself. Near term this is best done by the OS talking to a model host over a
  channel; true on-metal (and GPU-accelerated) inference is a long way out.
- **Voice I/O** (STT/TTS) and the **plugin system** — designed, not wired.

## Next milestones (suggested order)
1. Framebuffer font + a live command-center dashboard.
2. The syscall bridge + a minimal user-space process model.
3. Port the user-space Intelligence Router to run as the first OS service and
   talk to a model host — replacing the keyword planner with real model output
   behind the *same* pipeline.
