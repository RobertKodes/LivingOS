# LivingOS — status & honest roadmap

LivingOS is an experimental OS. This is a candid account of what actually works
today, kept honest on purpose. Everything below is verified booting in QEMU
(UEFI/OVMF) unless noted.

## What works today

**The Living Kernel (`kernel/`, `no_std` UEFI)**
- **Agents as first-class kernel objects** — identity, capabilities, lifecycle,
  priority, reputation; a **capability gate** that denies ungranted actions; a
  **priority scheduler**; a **transparent audit trail**; **inter-agent IPC**.
- **Interactive Living Shell** over keyboard *or* serial.
- **On-device planner** + **Living Memory** graph, **persisted across reboots**
  on a real FAT disk image (`tools/mkimage`).
- **GPU framebuffer** — boot splash + **visual command center** (`dash`) with an
  embedded bitmap font (`docs/splash.png`, `docs/command-center.png`).
- **Syscall bridge** — the kernel's own IDT; `int 0x80` traps into a kernel
  handler that dispatches by number (`syscall`).
- **Paging / memory management** — frame allocator + live virtual→physical
  page-table mapping; reads the UEFI memory map (`vm`).
- **On-metal neural-net inference** — a real char-level MLP runs *in the kernel*
  (`gen`; weights from `tools/train_nn.py`).
- **Audio + on-metal TTS** — PC-speaker tone synthesis; `beep`, `say`.
- **Plugin system** — agents loaded from an ESP `plugins.cfg`, capability-gated
  (`plugins`).
- **Networking** — drives a real NIC via the UEFI SNP: reads MAC/link (`net`),
  and a hand-rolled **IPv4/ICMP stack** that transmits valid ping frames —
  verified on the wire via QEMU `filter-dump` (`ping`).
- **Kernel↔user-space model bridge** — over COM2 to a host model service
  (`tools/model_bridge.py`) that routes to the local models; `ask` (and `hear`
  for STT) — verified round-trip. This is how real local-model intelligence
  reaches the on-OS agents, behind the same pipeline as the keyword planner.
- **Self-hosting** — `selfhost` calls **ExitBootServices** (firmware released)
  and runs LivingOS on its **own** IDT, COM1 serial, direct framebuffer writes,
  and PS/2 keyboard driver — no firmware beneath the kernel. Verified via the
  post-exit serial banner and a self-drawn framebuffer (`docs/selfhosted.png`).
- **Context-switch primitive** — `livingos_context_switch` (`task.rs`).

**User-space layer (`crates/`, host)** — the Agent Society, Intelligence Router
(Ollama + local image server), Living Memory, Eyes, Designer, and the `living`
CLI. Builds + unit-tested.

## Honest caveats (works, with limits)
- **Networking** — the IPv4/ICMP stack *transmits* correctly (verified on the
  wire), but *replies* aren't captured: OVMF's own UEFI network stack owns the
  SNP receive path in this setup, and QEMU's SLIRP NAT is selective about ARP/
  ICMP. A full TCP layer is not built (UDP/ICMP framing + checksums are).
- **Model bridge** — routes to local models via Ollama when present; falls back
  to a deterministic responder so it is demonstrable without Ollama installed.
- **On-metal model** — the in-kernel model is deliberately tiny; *large* models
  (and GPU acceleration) run host-side and are reached through the bridge, which
  is exactly the PRD's design (models live in user space).
- **Live multitasking** — the context-switch primitive works in isolation;
  preemptive timer-driven scheduling is not yet wired (the IDT it needs exists).
- **Self-hosting** is a one-way demonstration: after ExitBootServices it runs a
  minimal self-hosted loop (serial + framebuffer + PS/2), not the full shell.

## Roadmap
1. Port the user-space Intelligence Router to ride the model bridge so the real
   local specialist models drive the society end-to-end.
2. Stabilise live preemptive multitasking on the IDT timer.
3. Grow the IP stack: UDP/DNS, then a minimal TCP, over a self-owned NIC driver
   (post-self-hosting, bypassing OVMF's RX ownership).
4. A persistent self-hosted shell (keyboard + framebuffer compositor) after
   ExitBootServices.
