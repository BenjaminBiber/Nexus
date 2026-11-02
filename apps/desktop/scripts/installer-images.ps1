# Erzeugt die Grafiken des NSIS-Installers im Nexus-Look (Blau #1d4ed8, weißes H wie app-icon.svg):
#   src-tauri/windows/installer-sidebar.bmp (164x314, Willkommens- und Abschlussseite)
#   src-tauri/windows/installer-header.bmp  (150x57, Kopf der übrigen Seiten)
# Nur nach einer Änderung an Logo oder Farbe nötig; die BMPs liegen im Repo.
# Aufruf: powershell -NoProfile -ExecutionPolicy Bypass -File apps/desktop/scripts/installer-images.ps1

Add-Type -AssemblyName System.Drawing

$outDir = Join-Path $PSScriptRoot '..\src-tauri\windows'
$brand = [System.Drawing.ColorTranslator]::FromHtml('#1d4ed8')
$brandDeep = [System.Drawing.ColorTranslator]::FromHtml('#1e3a8a')
$white = [System.Drawing.Color]::White
$fontName = if ((New-Object System.Drawing.Text.InstalledFontCollection).Families.Name -contains 'Geist') { 'Geist' } else { 'Segoe UI Semibold' }

function New-Canvas([int]$w, [int]$h) {
  $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $g.TextRenderingHint = 'AntiAliasGridFit'
  $g.InterpolationMode = 'HighQualityBicubic'
  return $bmp, $g
}

function New-RoundedRect([single]$x, [single]$y, [single]$size, [single]$radius) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $radius * 2
  $p.AddArc($x, $y, $d, $d, 180, 90)
  $p.AddArc($x + $size - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $size - $d, $y + $size - $d, $d, $d, 0, 90)
  $p.AddArc($x, $y + $size - $d, $d, $d, 90, 90)
  $p.CloseFigure()
  return $p
}

# Das H aus app-icon.svg (1024er Raster: Striche bei 320/704, von 256 bis 768, Breite 112) im Feld x,y,size
function Draw-H($g, [single]$x, [single]$y, [single]$size, $color) {
  $s = $size / 1024
  $pen = New-Object System.Drawing.Pen $color, (112 * $s)
  $pen.StartCap = 'Round'; $pen.EndCap = 'Round'
  $g.DrawLine($pen, $x + 320 * $s, $y + 256 * $s, $x + 320 * $s, $y + 768 * $s)
  $g.DrawLine($pen, $x + 704 * $s, $y + 256 * $s, $x + 704 * $s, $y + 768 * $s)
  $g.DrawLine($pen, $x + 320 * $s, $y + 512 * $s, $x + 704 * $s, $y + 512 * $s)
  $pen.Dispose()
}

# Das ganze App-Icon: blaues Quadrat mit Radius 224/1024, darin das H
function Draw-AppIcon($g, [single]$x, [single]$y, [single]$size) {
  $path = New-RoundedRect $x $y $size ($size * 224 / 1024)
  $brush = New-Object System.Drawing.SolidBrush $brand
  $g.FillPath($brush, $path)
  $brush.Dispose(); $path.Dispose()
  Draw-H $g $x $y $size $white
}

# Seitenbild: Verlauf im Markenblau, großes H als Wasserzeichen unten rechts, Icon und Schriftzug oben
$bmp, $g = New-Canvas 164 314
$rect = New-Object System.Drawing.Rectangle 0, 0, 164, 314
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $brand, $brandDeep, 90
$g.FillRectangle($grad, $rect)
Draw-H $g 40 150 240 ([System.Drawing.Color]::FromArgb(28, 255, 255, 255))
$ring = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(70, 255, 255, 255)), 1.5
$g.DrawPath($ring, (New-RoundedRect 24 32 64 (64 * 224 / 1024)))
Draw-H $g 24 32 64 $white
$font = New-Object System.Drawing.Font $fontName, 22, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('Nexus', $font, (New-Object System.Drawing.SolidBrush $white), 20, 108)
$small = New-Object System.Drawing.Font $fontName, 11, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString("Board, Fragen und`nDispatcher f$([char]0xFC)r`nClaude Code", $small, (New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(200, 255, 255, 255))), 22, 140)
$bmp.Save((Join-Path $outDir 'installer-sidebar.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose(); $bmp.Dispose()

# Kopfbild: weißer Grund wie der Seitenkopf, App-Icon und Schriftzug in Markenblau
$bmp, $g = New-Canvas 150 57
$g.Clear($white)
Draw-AppIcon $g 10 10 37
$font = New-Object System.Drawing.Font $fontName, 20, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('Nexus', $font, (New-Object System.Drawing.SolidBrush $brand), 53, 14)
$bmp.Save((Join-Path $outDir 'installer-header.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose(); $bmp.Dispose()
