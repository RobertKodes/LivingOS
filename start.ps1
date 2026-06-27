# Boot LivingOS in a window you can type into and watch.
#
#   .\start.ps1            boot in a QEMU window (type in the window)
#   .\start.ps1 -Serial    boot headless, drive it from THIS terminal instead
#
# Needs QEMU + OVMF firmware. Builds the kernel first (needs Rust).
param([switch]$Serial)
$ErrorActionPreference = "Stop"
$repo = $PSScriptRoot
Set-Location $repo

Write-Host "==> building LivingOS"
Push-Location (Join-Path $repo "kernel"); cargo build --release; Pop-Location
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Push-Location (Join-Path $repo "tools\mkimage"); cargo build --release | Out-Null; Pop-Location

$efi = Join-Path $repo "kernel\target\x86_64-unknown-uefi\release\livingos.efi"
$img = Join-Path $repo "kernel\target\livingos.img"
& (Join-Path $repo "tools\mkimage\target\release\mkimage.exe") $img $efi | Out-Null

# Locate QEMU.
$q = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
$qemu = if ($q) { $q.Source } elseif (Test-Path "$env:USERPROFILE\qemu\qemu-system-x86_64.exe") { "$env:USERPROFILE\qemu\qemu-system-x86_64.exe" } else { $null }
if (-not $qemu) { Write-Host "QEMU not found. Install it or extract it to %USERPROFILE%\qemu."; exit 1 }
$qdir = Split-Path $qemu

# Locate split OVMF firmware and make a writable vars copy.
$code = @("$qdir\share\edk2-x86_64-code.fd", "$qdir\share\OVMF_CODE.fd", $env:OVMF_CODE) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
$varsSrc = @("$qdir\share\edk2-i386-vars.fd", "$qdir\share\OVMF_VARS.fd", $env:OVMF_VARS) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $code -or -not $varsSrc) { Write-Host "OVMF firmware not found. Set `$env:OVMF_CODE and `$env:OVMF_VARS."; exit 1 }
$vars = Join-Path $repo "kernel\target\vars.fd"; Copy-Item $varsSrc $vars -Force

$args = @(
    "-machine", "q35", "-m", "512", "-vga", "std",
    "-drive", "if=pflash,format=raw,readonly=on,file=$code",
    "-drive", "if=pflash,format=raw,file=$vars",
    "-drive", "format=raw,file=$img",
    "-no-reboot"
)
if ($Serial) {
    $args += @("-display", "none", "-serial", "stdio")
    Write-Host "==> booting LivingOS (drive it here; type 'help')"
} else {
    Write-Host "==> booting LivingOS in a window. Click it, then type 'help'."
    Write-Host "    Try:  ps   |   goal build a multiplayer game   |   dash   |   selfhost"
}
& $qemu @args
