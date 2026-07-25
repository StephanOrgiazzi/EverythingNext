param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$Background = [System.Drawing.ColorTranslator]::FromHtml("#FFD76B")
$Foreground = [System.Drawing.ColorTranslator]::FromHtml("#C95000")

function New-RoundedRectanglePath {
    param(
        [single]$X,
        [single]$Y,
        [single]$Width,
        [single]$Height,
        [single]$Radius
    )

    $diameter = [single]($Radius * 2)
    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
    $path.AddArc($X + $Width - $diameter, $Y, $diameter, $diameter, 270, 90)
    $path.AddArc($X + $Width - $diameter, $Y + $Height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($X, $Y + $Height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-IconBitmap {
    param([int]$Size)

    $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $inset = [single]($Size * 0.055)
    $radius = [single]($Size * 0.22)
    $path = New-RoundedRectanglePath -X $inset -Y $inset -Width ([single]($Size - 2 * $inset)) -Height ([single]($Size - 2 * $inset)) -Radius $radius
    $brush = [System.Drawing.SolidBrush]::new($Background)
    $graphics.FillPath($brush, $path)

    $strokeWidth = [single]([Math]::Max(2, $Size * 0.09))
    $pen = [System.Drawing.Pen]::new($Foreground, $strokeWidth)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round

    $graphics.DrawEllipse(
        $pen,
        [single]($Size * 0.22),
        [single]($Size * 0.20),
        [single]($Size * 0.40),
        [single]($Size * 0.40)
    )
    $graphics.DrawLine(
        $pen,
        [single]($Size * 0.58),
        [single]($Size * 0.57),
        [single]($Size * 0.77),
        [single]($Size * 0.76)
    )

    $pen.Dispose()
    $brush.Dispose()
    $path.Dispose()
    $graphics.Dispose()
    return ,$bitmap
}

function Get-PngBytes {
    param([System.Drawing.Bitmap]$Bitmap)

    $stream = [System.IO.MemoryStream]::new()
    try {
        $Bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        return $stream.ToArray()
    }
    finally {
        $stream.Dispose()
    }
}

function Write-Png {
    param(
        [int]$Size,
        [string]$Path
    )

    $bitmap = New-IconBitmap -Size $Size
    try {
        [System.IO.File]::WriteAllBytes($Path, (Get-PngBytes -Bitmap $bitmap))
    }
    finally {
        $bitmap.Dispose()
    }
}

function Write-Ico {
    param(
        [int[]]$Sizes,
        [string]$Path
    )

    $images = foreach ($size in $Sizes) {
        $bitmap = New-IconBitmap -Size $size
        try {
            [pscustomobject]@{
                Size = $size
                Bytes = [byte[]](Get-PngBytes -Bitmap $bitmap)
            }
        }
        finally {
            $bitmap.Dispose()
        }
    }

    $stream = [System.IO.MemoryStream]::new()
    $writer = [System.IO.BinaryWriter]::new($stream)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$images.Count)

        $offset = 6 + (16 * $images.Count)
        foreach ($image in $images) {
            $dimension = if ($image.Size -ge 256) { [byte]0 } else { [byte]$image.Size }
            $writer.Write($dimension)
            $writer.Write($dimension)
            $writer.Write([byte]0)
            $writer.Write([byte]0)
            $writer.Write([uint16]1)
            $writer.Write([uint16]32)
            $writer.Write([uint32]$image.Bytes.Length)
            $writer.Write([uint32]$offset)
            $offset += $image.Bytes.Length
        }

        foreach ($image in $images) {
            $writer.Write([byte[]]$image.Bytes)
        }

        $writer.Flush()
        [System.IO.File]::WriteAllBytes($Path, $stream.ToArray())
    }
    finally {
        $writer.Dispose()
        $stream.Dispose()
    }
}

$assetDirectory = Join-Path $Root "assets"
$tauriIconDirectory = Join-Path $Root "src-tauri/icons"
New-Item -ItemType Directory -Force -Path $assetDirectory, $tauriIconDirectory | Out-Null

Write-Png -Size 256 -Path (Join-Path $assetDirectory "icon.png")
Write-Png -Size 32 -Path (Join-Path $tauriIconDirectory "32x32.png")
Write-Png -Size 128 -Path (Join-Path $tauriIconDirectory "128x128.png")
Write-Png -Size 256 -Path (Join-Path $tauriIconDirectory "128x128@2x.png")
Write-Ico -Sizes @(16, 20, 24, 32, 40, 48, 64, 128, 256) -Path (Join-Path $tauriIconDirectory "icon.ico")

Write-Host "Generated Everything Modern icon assets."
