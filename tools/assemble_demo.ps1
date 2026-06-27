# Assemble the captured frames into a shareable MP4 with title and end cards.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$repo = Split-Path -Parent $PSScriptRoot
$vid = Join-Path $repo "release\video"
$frames = Join-Path $vid "frames"
$ffmpeg = (Get-Command ffmpeg).Source

function Card($path, $lines) {
    $w = 1280; $h = 720
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.TextRenderingHint = 'ClearTypeGridFit'
    $g.Clear([System.Drawing.Color]::FromArgb(12, 14, 20))
    $g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(24,110,200))), 0, 0, $w, 12)
    $g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(24,110,200))), 0, $h-12, $w, 12)
    $y = 210
    foreach ($ln in $lines) {
        $font = New-Object System.Drawing.Font($ln.Font, $ln.Size, $ln.Style)
        $brush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb($ln.R, $ln.G, $ln.B))
        $sz = $g.MeasureString($ln.Text, $font)
        $g.DrawString($ln.Text, $font, $brush, ($w - $sz.Width) / 2, $y)
        $y += $sz.Height + $ln.Gap
    }
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

$B = [System.Drawing.FontStyle]::Bold; $R = [System.Drawing.FontStyle]::Regular
Card (Join-Path $vid "title.png") @(
    @{Text="LivingOS"; Font="Segoe UI"; Size=84; Style=$B; R=240;G=244;B=255; Gap=18},
    @{Text="an AI-native operating system"; Font="Segoe UI"; Size=30; Style=$R; R=150;G=170;B=210; Gap=6},
    @{Text="agents are first-class, kernel-managed resources"; Font="Segoe UI"; Size=24; Style=$R; R=120;G=135;B=165; Gap=40},
    @{Text="a real Rust kernel - boots on bare metal (UEFI)"; Font="Consolas"; Size=22; Style=$R; R=90;G=200;B=255; Gap=0}
)
Card (Join-Path $vid "end.png") @(
    @{Text="LivingOS"; Font="Segoe UI"; Size=64; Style=$B; R=240;G=244;B=255; Gap=24},
    @{Text="open source - boots in a VM or on real hardware"; Font="Segoe UI"; Size=26; Style=$R; R=150;G=170;B=210; Gap=18},
    @{Text="github.com/RobertKodes/LivingOS"; Font="Consolas"; Size=30; Style=$B; R=90;G=200;B=255; Gap=0}
)

$enc = @("-c:v","libx264","-pix_fmt","yuv420p","-r","30","-vf","scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,setsar=1")
& $ffmpeg -y -loop 1 -i (Join-Path $vid "title.png") -t 3 @enc (Join-Path $vid "title.mp4") 2>$null
& $ffmpeg -y -framerate 5 -i (Join-Path $frames "frame-%04d.ppm") @enc (Join-Path $vid "body.mp4") 2>$null
& $ffmpeg -y -loop 1 -i (Join-Path $vid "end.png") -t 4 @enc (Join-Path $vid "end.mp4") 2>$null

$list = Join-Path $vid "list.txt"
"file 'title.mp4'`nfile 'body.mp4'`nfile 'end.mp4'" | Set-Content -Path $list -Encoding ascii
$outmp4 = Join-Path $vid "livingos_demo.mp4"
& $ffmpeg -y -f concat -safe 0 -i $list -c copy $outmp4 2>$null

# Also a looping GIF (X/Twitter friendly), ~12s.
$outgif = Join-Path $vid "livingos_demo.gif"
& $ffmpeg -y -i $outmp4 -vf "fps=10,scale=720:-1:flags=lanczos" $outgif 2>$null

"MP4: {0:N1} MB" -f ((Get-Item $outmp4).Length/1MB)
if (Test-Path $outgif) { "GIF: {0:N1} MB" -f ((Get-Item $outgif).Length/1MB) }
