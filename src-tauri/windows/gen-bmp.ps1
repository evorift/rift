Add-Type -AssemblyName System.Drawing

$outDir = $PSScriptRoot

# --- Header BMP (150 x 57) ---
$w = 150; $h = 57
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'ClearTypeGridFit'

# Dark gradient background
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point(0, 0)),
    (New-Object System.Drawing.Point($w, 0)),
    [System.Drawing.ColorTranslator]::FromHtml('#000000'),
    [System.Drawing.ColorTranslator]::FromHtml('#0b140d')
)
$g.FillRectangle($brush, 0, 0, $w, $h)

# Neon green accent line at bottom
$pen = New-Object System.Drawing.Pen([System.Drawing.ColorTranslator]::FromHtml('#39E66B'), 2)
$g.DrawLine($pen, 0, ($h - 2), $w, ($h - 2))

# evorift text
$font = New-Object System.Drawing.Font('Segoe UI', 16, [System.Drawing.FontStyle]::Bold)
$textBrush = New-Object System.Drawing.SolidBrush([System.Drawing.ColorTranslator]::FromHtml('#39E66B'))
$g.DrawString('evorift', $font, $textBrush, 8, 12)

# Subtle subtitle
$font2 = New-Object System.Drawing.Font('Segoe UI', 7, [System.Drawing.FontStyle]::Regular)
$textBrush2 = New-Object System.Drawing.SolidBrush([System.Drawing.ColorTranslator]::FromHtml('#5A615A'))
$g.DrawString('DPI Bypass Engine', $font2, $textBrush2, 10, 38)

$g.Dispose()
$headerPath = Join-Path $outDir 'header.bmp'
$bmp.Save($headerPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()
Write-Host "header.bmp created at $headerPath"

# --- Sidebar BMP (164 x 314) ---
$w2 = 164; $h2 = 314
$bmp2 = New-Object System.Drawing.Bitmap($w2, $h2)
$g2 = [System.Drawing.Graphics]::FromImage($bmp2)
$g2.SmoothingMode = 'AntiAlias'
$g2.TextRenderingHint = 'ClearTypeGridFit'

# Dark gradient background (top to bottom)
$brush2 = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.Point(0, 0)),
    (New-Object System.Drawing.Point(0, $h2)),
    [System.Drawing.ColorTranslator]::FromHtml('#000000'),
    [System.Drawing.ColorTranslator]::FromHtml('#0b140d')
)
$g2.FillRectangle($brush2, 0, 0, $w2, $h2)

# Neon green accent line on the right
$pen2 = New-Object System.Drawing.Pen([System.Drawing.ColorTranslator]::FromHtml('#39E66B'), 2)
$g2.DrawLine($pen2, ($w2 - 2), 0, ($w2 - 2), $h2)

# Subtle radial glow
for ($i = 60; $i -gt 0; $i -= 2) {
    $alpha = [int]([Math]::Max(0, 18 - ($i / 3)))
    $col = [System.Drawing.Color]::FromArgb($alpha, 57, 230, 107)
    $br = New-Object System.Drawing.SolidBrush($col)
    $cx = 82 - $i
    $cy = 120 - $i
    $g2.FillEllipse($br, $cx, $cy, ($i * 2), ($i * 2))
    $br.Dispose()
}

# evorift text centered
$font3 = New-Object System.Drawing.Font('Segoe UI', 14, [System.Drawing.FontStyle]::Bold)
$textBrush3 = New-Object System.Drawing.SolidBrush([System.Drawing.ColorTranslator]::FromHtml('#39E66B'))
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = 'Center'
$g2.DrawString('evorift', $font3, $textBrush3, (New-Object System.Drawing.RectangleF(0, 35, $w2, 40)), $sf)

# Version text
$font4 = New-Object System.Drawing.Font('Segoe UI', 8, [System.Drawing.FontStyle]::Regular)
$textBrush4 = New-Object System.Drawing.SolidBrush([System.Drawing.ColorTranslator]::FromHtml('#5A615A'))
$g2.DrawString('v0.1.0', $font4, $textBrush4, (New-Object System.Drawing.RectangleF(0, 60, $w2, 20)), $sf)

# Bottom tagline
$font5 = New-Object System.Drawing.Font('Segoe UI', 7, [System.Drawing.FontStyle]::Regular)
$textBrush5 = New-Object System.Drawing.SolidBrush([System.Drawing.ColorTranslator]::FromHtml('#9AA39A'))
$g2.DrawString('DPI Bypass Engine', $font5, $textBrush5, (New-Object System.Drawing.RectangleF(0, ($h2 - 30), $w2, 20)), $sf)

# Decorative dots (matrix rain feel)
$dotBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(30, 57, 230, 107))
$rng = New-Object System.Random(42)
for ($d = 0; $d -lt 40; $d++) {
    $dx = $rng.Next(0, $w2)
    $dy = $rng.Next(80, $h2 - 40)
    $ds = $rng.Next(1, 3)
    $g2.FillEllipse($dotBrush, $dx, $dy, $ds, $ds)
}

$g2.Dispose()
$sidebarPath = Join-Path $outDir 'sidebar.bmp'
$bmp2.Save($sidebarPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp2.Dispose()
Write-Host "sidebar.bmp created at $sidebarPath"
