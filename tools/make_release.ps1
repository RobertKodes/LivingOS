# Build a bootable, installable LivingOS image and convert it to common VM disk
# formats. Output lands in release/ at the repo root.
#
#   tools\make_release.ps1
#
# Produces:
#   release/livingos.img    raw UEFI-bootable disk (write to USB with Rufus/dd)
#   release/livingos.vhdx   Hyper-V / VirtualBox
#   release/livingos.vmdk   VMware / VirtualBox
#   release/livingos.qcow2  QEMU
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$kernel = Join-Path $repo "kernel"
$rel = Join-Path $repo "release"
New-Item -ItemType Directory -Force -Path $rel | Out-Null

Write-Host "==> building kernel (release)"
Push-Location $kernel
cargo build --release
Pop-Location
$efi = Join-Path $kernel "target\x86_64-unknown-uefi\release\livingos.efi"

Write-Host "==> building disk-image tool"
Push-Location (Join-Path $repo "tools\mkimage")
cargo build --release
Pop-Location
$mk = Join-Path $repo "tools\mkimage\target\release\mkimage.exe"

$img = Join-Path $rel "livingos.img"
if (Test-Path $img) { Remove-Item $img -Force }   # fresh, fixed-size image
& $mk $img $efi

# Locate qemu-img to convert to VM-friendly formats.
$qemuImg = (Get-Command qemu-img -ErrorAction SilentlyContinue).Source
if (-not $qemuImg) {
    $cand = "$env:USERPROFILE\qemu\qemu-img.exe"
    if (Test-Path $cand) { $qemuImg = $cand }
}
if ($qemuImg) {
    Write-Host "==> converting to VM formats with qemu-img"
    & $qemuImg convert -f raw -O vhdx -o subformat=dynamic $img (Join-Path $rel "livingos.vhdx")
    & $qemuImg convert -f raw -O vmdk $img (Join-Path $rel "livingos.vmdk")
    & $qemuImg convert -f raw -O qcow2 $img (Join-Path $rel "livingos.qcow2")
} else {
    Write-Host "qemu-img not found; only the raw .img was produced."
}

Write-Host "`n==> release artifacts:"
Get-ChildItem $rel | ForEach-Object { "  {0,-22} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB) }
Write-Host "`nSee docs/INSTALL.md to boot these in a VM or on real hardware."
