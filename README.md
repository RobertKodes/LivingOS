# LivingOS

**An AI-native operating system where intelligent agents are first-class,
kernel-managed resources** — alongside processes, threads, memory, and files.

Traditional operating systems are built around applications. LivingOS is built
around agents. You don't open apps; you express **goals**, and an agent society
assembles to accomplish them. The kernel understands agent **identity,
capabilities, lifecycle, scheduling, and IPC as native primitives** — not as an
application-level convention. The large language models that make agents
intelligent stay in user space (safer); the kernel owns the agents themselves.

This is an experimental OS. It boots on bare-metal UEFI firmware in QEMU.

```
 User  →  Goal  →  Agent Society  →  Living Kernel  →  Hardware
```

## Two layers

### 1. The Living Kernel — `kernel/`  (the operating system)
A `no_std`, bootable **UEFI** Rust kernel. `kernel/src/main.rs` is the OS image:
UEFI firmware loads `livingos.efi` and jumps straight into the kernel. At boot it
brings up its **native agent subsystem** and demonstrates the core machinery:

- agents as kernel objects (Agent Control Blocks) with scoped **capabilities**
- a **priority scheduler** over agents (the Native Agent Scheduler)
- a **capability gate** every privileged action must pass — granted *or denied*
- a transparent **audit trail** (every action is explainable)
- **reputation** that moves with outcomes (the Evolution Engine signal)

```sh
cd kernel
cargo build --release            # produces target/x86_64-unknown-uefi/release/livingos.efi
./run.ps1        # Windows: build + boot in QEMU (UEFI/OVMF)
./run.sh         # Linux/macOS: same
```

Boot output (in QEMU) shows the society spawning, the agent table, the
capability gate granting `screen_capture` to **Eyes** while **denying** it to
**Coder**, a goal being scheduled across specialists, the audit trail, and the
resulting reputation.

### 2. The user-space runtime — `crates/`  (the intelligence)
The agents' minds. In a full hardware build these run as a user-space system
service the kernel schedules; today they run on a host so you can drive the
society against **local models** right now.

| crate | role |
|---|---|
| `los-kernel` | host-side mirror of the agent/capability/scheduler model |
| `los-memory` | **Living Memory** — a persistent graph of goals, knowledge, observations |
| `los-router` | **Intelligence Router** — maps each agent role to a small local model (Ollama) + a local image server |
| `los-perception` | the **Eyes** — captures the desktop for the vision model |
| `los-runtime` | the **Agent Society** and the `goal` / `see` / `design` verbs |
| `los-shell` | `living` — the Living Shell (CLI) |

```sh
cargo build --release
./target/release/living init      # write config/ and data/
./target/release/living doctor    # check Ollama + pull missing models
./target/release/living ps        # see the society
./target/release/living goal   "build a snake game in python"
./target/release/living see    "what's on my screen right now?"
./target/release/living design "a neon koi fish in dark water, cinematic"
```

## The local model fleet (newest small specialists, mid-2026)
All local. No cloud. Edit `config/fleet.json` — the router is model-agnostic.

| role | model |
|---|---|
| conversation | `smollm3:3b` |
| planning | `gemma4:4b` |
| coding / tools | `qwen3.5:4b` |
| vision (Eyes) | `qwen3-vl:2b` |
| ocr | `glm-ocr:0.9b` |
| embedding | `embeddinggemma` |
| stt / tts | `moonshine` / `kokoro` |
| image (Designer) | **Z-Image Turbo** / **FLUX.2 Klein** (local SD server) |

## Build requirements
- Rust (stable) with the UEFI target: `rustup target add x86_64-unknown-uefi`
- QEMU + OVMF firmware (to boot the kernel)
- Ollama (for the user-space model fleet); a local Stable-Diffusion server for image gen

## Status & roadmap
**Milestone 1 (done):** bootable UEFI kernel with the native agent subsystem;
host-side Agent Runtime, Intelligence Router, Living Memory, the Eyes perception
agent, the Designer image agent, and the Living Shell — all building and tested.

**Next:** a framebuffer (GOP) GPU console and the visual command center; a
syscall boundary between the kernel and a user-space model runtime; embedding-
backed semantic memory; voice I/O; and persistence of the memory graph across
boots.

## License
MIT — see [LICENSE](LICENSE).
