#!/usr/bin/env bash
# Build LivingOS and boot it in QEMU under UEFI firmware (OVMF).
#
#   ./run.sh            # build + boot in a QEMU window
#   ./run.sh --serial   # headless: mirror the kernel console to this terminal
#
# Requires qemu-system-x86_64 and split OVMF firmware (a *-code.fd and a
# *-vars.fd). Override detection with OVMF_CODE / OVMF_VARS if needed.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> building livingos.efi (release)"
cargo build --release

efi="target/x86_64-unknown-uefi/release/livingos.efi"
esp="target/esp"
mkdir -p "$esp/EFI/BOOT"
cp "$efi" "$esp/EFI/BOOT/BOOTX64.EFI"
echo "==> EFI System Partition staged (EFI/BOOT/BOOTX64.EFI)"

if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
    echo "QEMU not found. Install qemu (apt install qemu-system-x86 / brew install qemu)."
    exit 1
fi

# Split OVMF: read-only CODE + a writable copy of VARS.
code="${OVMF_CODE:-}"
if [[ -z "$code" ]]; then
    for c in /usr/share/OVMF/OVMF_CODE.fd /usr/share/edk2/x64/OVMF_CODE.fd \
             /usr/share/qemu/edk2-x86_64-code.fd \
             /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
             /usr/local/share/qemu/edk2-x86_64-code.fd ; do
        [[ -f "$c" ]] && code="$c" && break
    done
fi
vars_src="${OVMF_VARS:-}"
if [[ -z "$vars_src" ]]; then
    for v in /usr/share/OVMF/OVMF_VARS.fd /usr/share/edk2/x64/OVMF_VARS.fd \
             /usr/share/qemu/edk2-i386-vars.fd \
             /opt/homebrew/share/qemu/edk2-i386-vars.fd \
             /usr/local/share/qemu/edk2-i386-vars.fd ; do
        [[ -f "$v" ]] && vars_src="$v" && break
    done
fi
if [[ -z "$code" || -z "$vars_src" ]]; then
    echo "OVMF firmware not found. Set OVMF_CODE and OVMF_VARS."
    exit 1
fi
cp "$vars_src" target/vars.fd
echo "==> firmware: $code (+ writable vars)"

qargs=(-machine q35 -m 256
       -drive "if=pflash,format=raw,readonly=on,file=$code"
       -drive "if=pflash,format=raw,file=target/vars.fd"
       -drive "format=raw,file=fat:rw:$esp"
       -no-reboot)
[[ "${1:-}" == "--serial" ]] && qargs+=(-display none -serial stdio)

echo "==> booting LivingOS in QEMU"
exec qemu-system-x86_64 "${qargs[@]}"
