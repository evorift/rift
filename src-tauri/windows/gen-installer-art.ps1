# gen-installer-art.ps1 — evorift NSIS installer icin dark-green (uygulama temasi) BMP gorselleri uret.
# header.bmp 150x57 (ust banner), sidebar.bmp 164x314 (welcome/finish sol panel). 24-bit BMP (NSIS uyumlu).
Add-Type -AssemblyName System.Drawing
$dir   = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\windows'
$logoP = 'C:\Users\Evrim\Desktop\projects\net\src-tauri\icons\icon.png'

$bgBase = [System.Drawing.Color]::FromArgb(6,8,6)       # ~#060806
$bgEl   = [System.Drawing.Color]::FromArgb(15,18,15)    # #0F120F
$green  = [System.Drawing.Color]::FromArgb(57,230,107)  # #39E66B accent
$muted  = [System.Drawing.Color]::FromArgb(150,180,150)

$logo = $null
try { $logo = [System.Drawing.Image]::FromFile($logoP) } catch { Write-Output "logo yuklenemedi ($logoP) — yalniz metin" }

function New-Bmp([int]$w,[int]$h,[string]$path,[scriptblock]$draw){
  $bmp = New-Object System.Drawing.Bitmap($w,$h,[System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $g.InterpolationMode = 'HighQualityBicubic'
  $g.TextRenderingHint = 'AntiAliasGridFit'
  & $draw $g
  $g.Dispose()
  $bmp.Save($path,[System.Drawing.Imaging.ImageFormat]::Bmp)
  $bmp.Dispose()
  Write-Output ("yazildi: $path (" + (Get-Item $path).Length + " byte)")
}

# ---- header 150x57: koyu zemin, solda 'evorift' yesil, sagda logo ----
New-Bmp 150 57 "$dir\installer-header.bmp" {
  param($g)
  $g.Clear($bgEl)
  if ($logo) { $g.DrawImage($logo, 150-52, 5, 47, 47) }
  $f  = New-Object System.Drawing.Font('Segoe UI',12,[System.Drawing.FontStyle]::Bold)
  $br = New-Object System.Drawing.SolidBrush($green)
  $g.DrawString('evorift',$f,$br,8,16)
  # ince yesil alt cizgi
  $pen = New-Object System.Drawing.Pen($green,2); $pen.Color=[System.Drawing.Color]::FromArgb(120,57,230,107)
  $g.DrawLine($pen,0,55,150,55)
}

# ---- sidebar 164x314: dikey gradient, ustte logo, 'evorift' + slogan, altta yesil cizgi ----
New-Bmp 164 314 "$dir\installer-sidebar.bmp" {
  param($g)
  $rect = New-Object System.Drawing.Rectangle(0,0,164,314)
  $lg = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect,$bgEl,$bgBase,90.0)
  $g.FillRectangle($lg,$rect)
  if ($logo) { $g.DrawImage($logo, [int]((164-96)/2), 40, 96, 96) }
  $sf = New-Object System.Drawing.StringFormat; $sf.Alignment='Center'
  $f1 = New-Object System.Drawing.Font('Segoe UI',19,[System.Drawing.FontStyle]::Bold)
  $brG = New-Object System.Drawing.SolidBrush($green)
  $g.DrawString('evorift',$f1,$brG,(New-Object System.Drawing.RectangleF(0,150,164,32)),$sf)
  $f2 = New-Object System.Drawing.Font('Segoe UI',8.5)
  $brM = New-Object System.Drawing.SolidBrush($muted)
  $g.DrawString('DPI bypass — VPN yok',$f2,$brM,(New-Object System.Drawing.RectangleF(0,184,164,20)),$sf)
  $g.DrawString('Discord · Roblox · oyun',$f2,$brM,(New-Object System.Drawing.RectangleF(0,200,164,20)),$sf)
  $pen = New-Object System.Drawing.Pen($green,2)
  $g.DrawLine($pen,24,294,140,294)
}

if ($logo) { $logo.Dispose() }
Write-Output "TAMAM"
