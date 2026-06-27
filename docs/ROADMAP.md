# LivingOS roadmap — toward a visual, self-hosted AI-native desktop

Where we are: LivingOS boots on UEFI (VM or hardware), the agent society and
capability kernel run on it, it has a GPU framebuffer with a command-center
*view* (`dash`), a model bridge to local models, and it can take the machine
from the firmware (`selfhost`). The two big arcs ahead are **a real graphical
GUI** and **a fully self-hosted desktop the agents live in**.

## A. The visual GUI (the command center, made live)

Today `dash` paints a one-shot framebuffer snapshot. The goal is a **live,
always-on, GPU-rendered command center** — the PRD's "watch intelligence
working" interface — where you see agents, conversations, the memory graph, and
task pipelines update in real time, and drive everything by goals.

Plan, in shippable stages:

1. **Framebuffer compositor** (`gui/` module).
   - A double-buffered backbuffer (draw off-screen, blit on vsync) over the GOP
     framebuffer we already grab in `selfhost.rs`.
   - Primitives: filled rects, lines, the existing 8×16 bitmap font (add a 2×/3×
     scale and a bold weight), rounded panels, simple icons.
   - A dirty-rect redraw loop so it's cheap.

2. **Window/panel layout** (a tiling dashboard, not overlapping windows).
   - Left: the **agent society** as live cards (name, state, reputation bar,
     current task) — `dash` already prototypes this.
   - Center: the **goal feed / conversation** — what you typed, the plan, the
     agents' messages (`msgs`) streaming in.
   - Right: the **memory graph** (nodes as dots, edges as lines, force-ish
     layout) and **system meters** (RAM, model usage, audit count).
   - Bottom: a command bar (the shell, but graphical).

3. **Input.** Reuse the PS/2 keyboard driver from `selfhost.rs` for typing;
   add a PS/2 **mouse** driver (port 0x60, mouse packets) for clicking cards and
   panning the graph.

4. **Event loop.** Replace the text REPL with a GUI loop: poll keyboard/mouse,
   advance agent work a step, recomposite. Agents run as kernel tasks (needs the
   preemptive scheduler below) so the UI stays responsive while they think.

5. **Theming + polish.** The brand palette is already in the splash/`dash`.
   Add animation (reputation bars easing, message cards sliding in) — cheap with
   the compositor.

Milestone: boot → graphical command center, type a goal, watch the society light
up and the memory graph grow, all on the framebuffer.

## B. The self-hosted desktop (no firmware)

`selfhost` already exits boot services and runs on our own IDT + serial +
framebuffer + PS/2. To make it the real OS:

1. **Persistent self-hosted shell/GUI** — run the GUI (above) *after*
   ExitBootServices, not the one-shot demo loop. We already have framebuffer +
   keyboard there.
2. **Preemptive multitasking** — wire the IDT timer (PIT/APIC) so agents are
   real preempted tasks; this also likely fixes the context-switch bootstrap
   that's flaky under OVMF (no firmware to fight). Agents then think in the
   background while the GUI runs.
3. **Own disk driver** (AHCI/virtio-blk) + a simple FS, so Living Memory and
   plugins persist without UEFI's filesystem.
4. **Own NIC driver** (virtio-net/e1000 RX) so the IPv4 stack gets real
   round-trips (ARP/ICMP/UDP/DNS), unlocking the `Internet` capability — and the
   model bridge can run over the network instead of COM2.

## C. Intelligence that lives there

1. **Wire the planner to the model bridge end-to-end** (started): `goal` already
   consults the local model; next, have each specialist agent's step call the
   right local model (coder→a code model, vision→a VLM) through the router.
2. **On-device acceleration** — keep the tiny in-kernel model for instant
   reflexes; route heavy reasoning to the host model service (GPU) over the
   bridge. Long-term: a userland + GPU driver for on-metal inference.
3. **The Evolution Engine** — persist reputation across boots (we persist memory
   already) and actually route work to higher-reputation agents.

## Suggested order
1. Framebuffer compositor + live command center (A1–A2) — most visible.
2. Preemptive scheduler (B2) — unblocks responsive agents and fixes multitasking.
3. PS/2 mouse + input (A3) and the persistent self-hosted GUI (B1).
4. Own disk + NIC drivers (B3–B4).
5. Full router-driven specialist agents (C1).
