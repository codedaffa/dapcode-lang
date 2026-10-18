Add-Type -AssemblyName System.Drawing

$width = 256
$height = 256
$bmp = New-Object System.Drawing.Bitmap($width, $height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

# Clear with transparent (No background, No frame)
$g.Clear([System.Drawing.Color]::Transparent)

# Glyph path for {D}
$gp = New-Object System.Drawing.Drawing2D.GraphicsPath
$ff = New-Object System.Drawing.FontFamily('Consolas')
$sf = New-Object System.Drawing.StringFormat([System.Drawing.StringFormat]::GenericTypographic)
$sf.FormatFlags = $sf.FormatFlags -bor [System.Drawing.StringFormatFlags]::NoWrap -bor [System.Drawing.StringFormatFlags]::FitBlackBox
$gp.AddString('{D}', $ff, [int][System.Drawing.FontStyle]::Bold, 100, [System.Drawing.PointF]::new(0, 0), $sf)

$bounds = $gp.GetBounds()
$avail = 256.0 * 0.92
$scale = [Math]::Min($avail / $bounds.Width, $avail / $bounds.Height)
$finalW = $bounds.Width * $scale
$finalH = $bounds.Height * $scale
$offX = ((256.0 - $finalW) / 2.0) - ($bounds.X * $scale)
$offY = ((256.0 - $finalH) / 2.0) - ($bounds.Y * $scale)

$matrix = New-Object System.Drawing.Drawing2D.Matrix
$matrix.Translate($offX, $offY)
$matrix.Scale($scale, $scale)
$gp.Transform($matrix)

# 135deg Gradient: Cyan -> Indigo -> Purple
$pt1 = New-Object System.Drawing.PointF 0.0, 0.0
$pt2 = New-Object System.Drawing.PointF 256.0, 256.0
$c1 = [System.Drawing.Color]::FromArgb(255, 56, 189, 248)
$c2 = [System.Drawing.Color]::FromArgb(255, 192, 132, 252)
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush ($pt1, $pt2, $c1, $c2)
$cb = New-Object System.Drawing.Drawing2D.ColorBlend 3
$cb.Colors = @(
    [System.Drawing.Color]::FromArgb(255, 56, 189, 248),
    [System.Drawing.Color]::FromArgb(255, 129, 140, 248),
    [System.Drawing.Color]::FromArgb(255, 192, 132, 252)
)
$cb.Positions = [float[]]@(0.0, 0.45, 1.0)
$brush.InterpolationColors = $cb

$g.FillPath($brush, $gp)

$outDir = Join-Path $PSScriptRoot "icons"
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force }
$pngPath = Join-Path $outDir "dapcode.png"
$bmp.Save($pngPath, [System.Drawing.Imaging.ImageFormat]::Png)

$brush.Dispose()
$g.Dispose()
$bmp.Dispose()
Write-Host "Saved pure {D} PNG to $pngPath"
