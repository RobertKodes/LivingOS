# Using LivingOS

## Boot it

From the project folder:

```powershell
.\start.ps1
```

This builds the kernel, makes a bootable image, and opens a **QEMU window**. You'll
see the GPU boot splash, then it clears into the **Living Shell**. Click the
window so it has keyboard focus, then type.

(Prefer your terminal? `.\start.ps1 -Serial` boots headless and you drive it from
the terminal instead — same shell, no window.)

## A good first session (nice to record)

```
help                              # the command list
ps                                # the agent society: 12 agents + capabilities
goal build a multiplayer game     # watch the society plan, schedule, collaborate
msgs                              # the kernel-routed messages the agents exchanged
mem                               # Living Memory (persists across reboots)
dash                              # the visual command center (GPU framebuffer)
syscall                           # int 0x80 traps into the kernel's own IDT
vm                                # frame allocator + live page-table mapping
gen the kernel                    # a neural net running *in the kernel*
net                               # drive the NIC; read its MAC
ping                              # build + send a real IPv4/ICMP packet
selfhost                          # the finale: take the machine from the firmware
```

`selfhost` is one-way: it calls `ExitBootServices` and runs LivingOS on its own
IDT + serial + framebuffer + PS/2 drivers — "no firmware beneath the kernel."
Close the window when done.

## Drive it with a real local model (optional)

`goal` and `ask` can route to local models through the model bridge:

1. Install [Ollama](https://ollama.com) and `ollama pull qwen2.5:0.5b`.
2. Boot with a bridge channel and run the daemon:
   ```powershell
   # boot LivingOS exposing COM2 as a socket (and a window):
   qemu-system-x86_64 -machine q35 -m 512 -vga std `
     -drive if=pflash,format=raw,readonly=on,file=<OVMF_CODE.fd> `
     -drive if=pflash,format=raw,file=<OVMF_VARS.fd> `
     -drive format=raw,file=kernel\target\livingos.img `
     -serial tcp:127.0.0.1:4555,server,nowait
   # in another terminal:
   python tools\model_bridge.py 127.0.0.1 4555
   ```
3. In LivingOS: `ask build a snake game` → the local model answers. (CPU-only
   inference is slow; a GPU makes it instant.)

Without a daemon, `goal` just uses the fast on-device planner — it never blocks.
