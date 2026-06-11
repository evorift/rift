# gen-installer-art.ps1 - evorift NSIS installer gorselleri (SADE, SIYAH zemin, logo + wordmark).
# Tasarim: saf siyah (#000000) zemin, ortada [logo][evorift] yatay kilit, ince yesil aksan cizgisi.
# header.bmp 150x57 (inner sayfa sag-ust banner), sidebar.bmp 164x314 (welcome/finish sol panel).
# 24-bit BMP (NSIS uyumlu). tauri.conf.json bundle.windows.nsis bu adlari referans alir.
Add-Type -AssemblyName System.Drawing
$dir   = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\windows'
$logoP = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\icons\logo.png'

$black = [System.Drawing.Color]::FromArgb(0,0,0)        # saf siyah zemin
$green = [System.Drawing.Color]::FromArgb(57,230,107)   # #39E66B aksan
$muted = [System.Drawing.Color]::FromArgb(120,150,120)  # silik slogan

$logo = $null
try { $logo = [System.Drawing.Image]::FromFile($logoP) } catch { Write-Output "logo yuklenemedi ($logoP) - yalniz metin" }

function New-Bmp([int]$w,[int]$h,[string]$path,[scriptblock]$draw){
  $bmp = New-Object System.Drawing.Bitmap($w,$h,[System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $g.InterpolationMode = 'HighQualityBicubic'
  $g.TextRenderingHint = 'AntiAliasGridFit'
  $g.PixelOffsetMode = 'HighQuality'
  & $draw $g
  $g.Dispose()
  $bmp.Save($path,[System.Drawing.Imaging.ImageFormat]::Bmp)
  $bmp.Dispose()
  Write-Output ("yazildi: $path (" + (Get-Item $path).Length + " byte)")
}

# ---- header 150x57: siyah zemin, ortada [logo][evorift] kilit, altta ince yesil cizgi ----
New-Bmp 150 57 "$dir\header.bmp" {
  param($g)
  $g.Clear($black)
  $f  = New-Object System.Drawing.Font('Segoe UI',13,[System.Drawing.FontStyle]::Bold)
  $brG = New-Object System.Drawing.SolidBrush($green)
  $logoW = 30
  $gap   = 6
  $textSz = $g.MeasureString('evorift',$f)
  $total  = $logoW + $gap + $textSz.Width
  $startX = [int](((150 - $total) / 2))
  $cy = 26   # dikey orta
  if ($logo) { $g.DrawImage($logo, $startX, ($cy - [int]($logoW/2)), $logoW, $logoW) }
  $g.DrawString('evorift',$f,$brG, ($startX + $logoW + $gap), ($cy - ($textSz.Height/2)))
  # ince yesil aksan cizgisi (alt)
  $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(160,57,230,107),2)
  $g.DrawLine($pen,0,55,150,55)
}

# ---- sidebar 164x314: siyah zemin, ortada [logo][evorift] kilit + silik slogan, altta yesil cizgi ----
New-Bmp 164 314 "$dir\sidebar.bmp" {
  param($g)
  $g.Clear($black)
  $f1  = New-Object System.Drawing.Font('Segoe UI',16,[System.Drawing.FontStyle]::Bold)
  $brG = New-Object System.Drawing.SolidBrush($green)
  $logoW = 40
  $gap   = 8
  $textSz = $g.MeasureString('evorift',$f1)
  $total  = $logoW + $gap + $textSz.Width
  $startX = [int](((164 - $total) / 2))
  $cy = 150   # dikey orta
  if ($logo) { $g.DrawImage($logo, $startX, ($cy - [int]($logoW/2)), $logoW, $logoW) }
  $g.DrawString('evorift',$f1,$brG, ($startX + $logoW + $gap), ($cy - ($textSz.Height/2)))
  # silik slogan (kilidin altinda, ortali)
  $f2 = New-Object System.Drawing.Font('Segoe UI',8.5)
  $brM = New-Object System.Drawing.SolidBrush($muted)
  $sf = New-Object System.Drawing.StringFormat; $sf.Alignment='Center'
  $g.DrawString('DPI bypass - VPN yok',$f2,$brM,(New-Object System.Drawing.RectangleF(0,182,164,20)),$sf)
  # ince yesil aksan cizgisi (alt)
  $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(160,57,230,107),2)
  $g.DrawLine($pen,32,292,132,292)
}

if ($logo) { $logo.Dispose() }
Write-Output "TAMAM"
