# Build LivingOS and boot it in QEMU under UEFI firmware (OVMF).
#
#   .\run.ps1            # build + boot in a QEMU window
#   .\run.ps1 -Serial    # headless: mirror the kernel console to this terminal
#
# Requires QEMU and split OVMF firmware (an *-code.fd and a *-vars.fd). The
# QEMU Windows build ships these in its share\ directory. Override detection
# with $env:OVMF_CODE and $env:OVMF_VARS if needed.
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
Write-Host "==> EFI System Partition staged (EFI\BOOT\BOOTX64.EFI)"

$qemu = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
if (-not $qemu) {
    $local = "$env:USERPROFILE\qemu\qemu-system-x86_64.exe"
    if (Test-Path $local) { $qemu = Get-Item $local } else {
        Write-Host "QEMU not found. Install it (winget install SoftwareFreedomConservancy.QEMU)"
        Write-Host "or extract the qemu.weilnetz.de installer to %USERPROFILE%\qemu."
        exit 1
    }
}
$qdir = Split-Path $qemu.Source

# Split OVMF: read-only CODE + a writable copy of VARS.
$code = $env:OVMF_CODE
if (-not $code) {
    $code = @("$qdir\share\edk2-x86_64-code.fd", "$qdir\share\OVMF_CODE.fd") |
            Where-Object { Test-Path $_ } | Select-Object -First 1
}
$varsSrc = $env:OVMF_VARS
if (-not $varsSrc) {
    $varsSrc = @("$qdir\share\edk2-i386-vars.fd", "$qdir\share\OVMF_VARS.fd") |
               Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $code -or -not $varsSrc) {
    Write-Host "OVMF firmware not found. Set `$env:OVMF_CODE and `$env:OVMF_VARS."
    exit 1
}
$vars = "target\vars.fd"
Copy-Item $varsSrc $vars -Force
Write-Host "==> firmware: $code (+ writable vars)"

$qargs = @(
    "-machine", "q35", "-m", "256",
    "-drive", "if=pflash,format=raw,readonly=on,file=$code",
    "-drive", "if=pflash,format=raw,file=$vars",
    "-drive", "format=raw,file=fat:rw:$esp",
    "-no-reboot"
)
if ($Serial) { $qargs += @("-display", "none", "-serial", "stdio") }

Write-Host "==> booting LivingOS in QEMU"
& $qemu.Source @qargs
