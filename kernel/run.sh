#!/usr/bin/env bash
# Build LivingOS and boot it in QEMU under UEFI firmware (OVMF).
#
#   ./run.sh            # build + boot in a QEMU window
#   ./run.sh --serial   # mirror the kernel console to this terminal
#
# Requires qemu-system-x86_64 and an OVMF firmware image. Set OVMF=/path to
# OVMF_CODE.fd if auto-detect fails.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> building livingos.efi (release)"
cargo build --release

efi="target/x86_64-unknown-uefi/release/livingos.efi"
esp="target/esp"
mkdir -p "$esp/EFI/BOOT"
cp "$efi" "$esp/EFI/BOOT/BOOTX64.EFI"
echo "==> EFI System Partition staged at $esp"

if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
    echo "QEMU not found. Install qemu (e.g. 'sudo apt install qemu-system-x86' or 'brew install qemu')."
    exit 1
fi

ovmf="${OVMF:-}"
if [[ -z "$ovmf" || ! -f "$ovmf" ]]; then
    for c in \
        /usr/share/OVMF/OVMF_CODE.fd \
        /usr/share/ovmf/OVMF.fd \
        /usr/share/edk2/x64/OVMF_CODE.fd \
        /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
        /usr/local/share/qemu/edk2-x86_64-code.fd ; do
        [[ -f "$c" ]] && ovmf="$c" && break
    done
fi
if [[ -z "$ovmf" ]]; then
    echo "OVMF firmware not found. Install ovmf and/or set OVMF=/path/to/OVMF_CODE.fd"
    exit 1
fi
echo "==> firmware: $ovmf"

qargs=(-machine q35 -m 256 -bios "$ovmf" -drive "format=raw,file=fat:rw:$esp")
[[ "${1:-}" == "--serial" ]] && qargs+=(-serial stdio)

echo "==> booting LivingOS in QEMU"
exec qemu-system-x86_64 "${qargs[@]}"
