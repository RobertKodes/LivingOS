# Build LivingOS and boot it in QEMU under UEFI firmware (OVMF).
#
#   .\run.ps1            # build + boot in a QEMU window
#   .\run.ps1 -Serial    # mirror the kernel console to this terminal
#
# Requires QEMU (winget install SoftwareFreedomConservancy.QEMU) and an OVMF
# firmware image. Set $env:OVMF to point at OVMF_CODE.fd if auto-detect fails.
param([switch]$Serial)

$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

Write-Host "==> building livingos.efi (release)"
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$efi = "target\x86_64-unknown-uefi\release\livingos.efi"
$esp = "target\esp"
New-Item -ItemType Directory -Force -Path "$esp\EFI\BOOT" | Out-Null
Copy-Item $efi "$esp\EFI\BOOT\BOOTX64.EFI" -Force
Write-Host "==> EFI System Partition staged at $esp (EFI\BOOT\BOOTX64.EFI)"

$qemu = (Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue)
if (-not $qemu) {
    Write-Host "QEMU not found. Install it with:"
    Write-Host "    winget install SoftwareFreedomConservancy.QEMU"
    exit 1
}

# Locate OVMF firmware: $env:OVMF, then common QEMU share locations.
$ovmf = $env:OVMF
if (-not $ovmf -or -not (Test-Path $ovmf)) {
    $qdir = Split-Path $qemu.Source
    $cands = @(
        "$qdir\share\edk2-x86_64-code.fd",
        "$qdir\share\OVMF_CODE.fd",
        "$qdir\OVMF.fd",
        "$qdir\edk2-x86_64-code.fd"
    )
    $ovmf = $cands | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $ovmf) {
    Write-Host "OVMF firmware not found. Download OVMF (edk2) and set:"
    Write-Host "    `$env:OVMF = 'C:\path\to\OVMF_CODE.fd'"
    exit 1
}
Write-Host "==> firmware: $ovmf"

$args = @(
    "-machine", "q35",
    "-m", "256",
    "-bios", $ovmf,
    "-drive", "format=raw,file=fat:rw:$esp"
)
if ($Serial) { $args += @("-serial", "stdio") }

Write-Host "==> booting LivingOS in QEMU"
& $qemu.Source @args
