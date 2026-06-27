# Record a LivingOS demonstration: boot in QEMU, drive a scripted session, and
# capture framebuffer frames via QMP. A companion ffmpeg step assembles the MP4.
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path $repo "kernel\target"
$qemu = "C:\Users\kodesweb3\qemu\qemu-system-x86_64.exe"
$code = "C:\Users\kodesweb3\qemu\share\edk2-x86_64-code.fd"
$frames = Join-Path $repo "release\video\frames"
if (Test-Path $frames) { Remove-Item $frames -Recurse -Force }
New-Item -ItemType Directory -Force -Path $frames | Out-Null

# Fresh image with the latest kernel.
$img = Join-Path $work "livingos.img"
& (Join-Path $repo "tools\mkimage\target\release\mkimage.exe") $img (Join-Path $work "x86_64-unknown-uefi\release\livingos.efi") | Out-Null
Copy-Item "C:\Users\kodesweb3\qemu\share\edk2-i386-vars.fd" (Join-Path $work "vars.fd") -Force

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $qemu; $psi.WorkingDirectory = $work
$psi.Arguments = "-machine q35 -m 512 -vga std -drive if=pflash,format=raw,readonly=on,file=$code -drive if=pflash,format=raw,file=vars.fd -drive format=raw,file=livingos.img -serial stdio -serial tcp:127.0.0.1:4580,server,nowait -qmp tcp:127.0.0.1:5580,server,nowait -display none -no-reboot"
$psi.RedirectStandardInput = $true; $psi.RedirectStandardOutput = $true; $psi.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($psi)
$p.BeginOutputReadLine() | Out-Null
Start-Sleep -Seconds 2
$dp = Start-Process python -ArgumentList "`"$repo\tools\model_bridge.py`"","127.0.0.1","4580","offline" -PassThru -WindowStyle Hidden

# QMP
$cli = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 5580)
$ns = $cli.GetStream(); $rd = New-Object IO.StreamReader($ns); $wr = New-Object IO.StreamWriter($ns); $wr.AutoFlush = $true
Start-Sleep -Milliseconds 300; $null = $rd.ReadLine()
$wr.WriteLine('{"execute":"qmp_capabilities"}'); Start-Sleep -Milliseconds 200; $null = $rd.ReadLine()

$script:n = 0
function Cap {
    $script:n++
    $f = (Join-Path $frames ("frame-{0:D4}.ppm" -f $script:n)) -replace '\\','/'
    $wr.WriteLine('{"execute":"screendump","arguments":{"filename":"' + $f + '"}}')
    Start-Sleep -Milliseconds 120; $null = $rd.ReadLine()
}
function Burst($count, $ms) { for ($i=0; $i -lt $count; $i++) { Cap; Start-Sleep -Milliseconds $ms } }
function SendCmd($s) { foreach ($ch in $s.ToCharArray()) { $p.StandardInput.Write($ch); $p.StandardInput.Flush(); Start-Sleep -Milliseconds 12 }; $p.StandardInput.WriteLine(""); $p.StandardInput.Flush() }
function SendEnter { $p.StandardInput.WriteLine(""); $p.StandardInput.Flush() }

Start-Sleep -Seconds 4          # firmware + boot
Burst 10 260                    # GPU boot splash (emblem + agent indicators)
# Run a goal: the society works (on serial); it updates agent reputation/memory,
# which the command center then visualises.
SendCmd "goal build a multiplayer game"; Start-Sleep -Seconds 5
SendCmd "dash"; Start-Sleep -Milliseconds 1500; Burst 18 280    # visual command center (the agent society)
SendCmd "selfhost"; Start-Sleep -Seconds 3; Burst 16 300        # ExitBootServices finale

if (-not $p.HasExited) { $p.Kill() }; if (-not $dp.HasExited) { $dp.Kill() }; $cli.Close()
"captured $script:n frames in $frames"
