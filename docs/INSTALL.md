# Installing / booting LivingOS

LivingOS boots on any **UEFI** machine or VM (x86-64). It's a single bootable
disk image — there's no installer to run; you boot the image directly. The agent
society, capability kernel, memory, and shell all live on it.

Build the images yourself with:

```powershell
tools\make_release.ps1
```

which produces, in `release/`:

| File | Use with |
|---|---|
| `livingos.img` | raw disk — write to a USB stick, or attach to any VM |
| `livingos.vhdx` | Hyper-V, VirtualBox |
| `livingos.vmdk` | VMware, VirtualBox |
| `livingos.qcow2` | QEMU |

> The whole OS is ~128 KB. The image is 64 MiB only to leave room for the
> persistent Living Memory the kernel writes back to it.

## Boot in a VM

**QEMU** (what we develop against):
```sh
qemu-system-x86_64 -machine q35 -m 512 \
  -drive if=pflash,format=raw,readonly=on,file=OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS.fd \
  -drive format=raw,file=release/livingos.img
```
(or just `kernel\run.ps1`). A serial console is handy: add `-serial stdio`.

**VirtualBox**: New VM → type *Other/Unknown (64-bit)* → in **Settings → System**
enable **EFI** → **Storage**: attach `livingos.vmdk` (or `.vhdx`) as the disk →
Start.

**VMware**: New VM → *I will install later* → set firmware to **UEFI** → replace
the disk with `livingos.vmdk` → Power on.

**Hyper-V**: New VM → **Generation 2** (Generation 2 is UEFI) → attach
`livingos.vhdx` → in Security, turn **Secure Boot off** → Start.

## Boot on real hardware (USB)

1. Write `livingos.img` to a USB stick (this **erases** the stick):
   - **Windows**: use [Rufus](https://rufus.ie) → select `livingos.img` →
     *DD image* mode → Start. (Or `dd` from WSL/Git-Bash.)
   - **Linux/macOS**: `sudo dd if=release/livingos.img of=/dev/sdX bs=4M conv=fsync`
     (replace `/dev/sdX` with your USB device — double-check it!).
2. Reboot the target machine, enter the firmware boot menu (F12 / F10 / Esc,
   varies), and pick the USB device under **UEFI**.
3. Turn **Secure Boot off** in firmware if the loader is rejected (the kernel is
   not signed).

LivingOS boots straight into the Living Shell. Type `help`, then state a goal:
`goal build a multiplayer game`. Try `dash` (visual command center), `ps`,
`syscall`, `vm`, `net`, and — to have the kernel take the machine fully from the
firmware — `selfhost`.

## Notes
- It boots **UEFI only** (no legacy BIOS/CSM). Enable UEFI in firmware.
- It is harmless to your data: it boots from the USB/VM disk and only writes its
  own memory file to that image — it does not touch other drives.
