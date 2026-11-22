# Renders the keyboard's black-and-white icons (docs/keyboard/PLAN.md 7) in the style of the Microsoft IME requirements page
# ("IME icons"): sizes 16, 20, 24, 32, 40 and 48 px, 32-bit with alpha, PNG-compressed ICO entries.
#   brand.ico     a white box with a black glyph, 1 px outer stroke in black at 50 % opacity (the Input Indicator's brand icon)
#   mode-on.ico   a white glyph with a 1 px black 50 % outline: conversion on  (ş)
#   mode-off.ico  the same for conversion off (s)
# Run once after a design change and commit the .ico files: powershell -NoProfile -File res/make-icons.ps1
Add-Type -AssemblyName System.Drawing
$Out = $PSScriptRoot
$Sizes = 16, 20, 24, 32, 40, 48
$Stroke = [System.Drawing.Color]::FromArgb(128, 0, 0, 0)

function Glyph-Path([string]$text, [int]$px, [float]$em) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $fam = New-Object System.Drawing.FontFamily 'Segoe UI'
    $fmt = [System.Drawing.StringFormat]::GenericTypographic
    $p.AddString($text, $fam, [int][System.Drawing.FontStyle]::Bold, $em, (New-Object System.Drawing.PointF 0, 0), $fmt)
    # centre the glyph's ink box in the square
    $b = $p.GetBounds()
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Translate(($px - $b.Width) / 2 - $b.X, ($px - $b.Height) / 2 - $b.Y)
    $p.Transform($m)
    return $p
}

function Render([int]$px, [string]$kind, [string]$text) {
    $bmp = New-Object System.Drawing.Bitmap $px, $px, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([System.Drawing.Color]::Transparent)
    $pen = New-Object System.Drawing.Pen $Stroke, 1
    if ($kind -eq 'brand') {
        $r = New-Object System.Drawing.RectangleF 0.5, 0.5, ($px - 1), ($px - 1)
        $g.FillRectangle([System.Drawing.Brushes]::White, $r)
        $g.DrawRectangle($pen, $r.X, $r.Y, $r.Width, $r.Height)
        $path = Glyph-Path $text $px ($px * 0.78)
        $g.FillPath([System.Drawing.Brushes]::Black, $path)
    } else {
        $path = Glyph-Path $text $px ($px * 1.0)
        $pen.LineJoin = 'Round'
        $g.DrawPath((New-Object System.Drawing.Pen $Stroke, 2), $path)  # 2 px pen = 1 px outside the fill
        $g.FillPath([System.Drawing.Brushes]::White, $path)
    }
    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return , $ms.ToArray()
}

function Write-Ico([string]$name, [string]$kind, [string]$text) {
    $imgs = foreach ($s in $Sizes) { , (Render $s $kind $text) }
    $fs = [System.IO.File]::Create((Join-Path $Out $name))
    $w = New-Object System.IO.BinaryWriter $fs
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$Sizes.Count)
    $off = 6 + 16 * $Sizes.Count
    for ($i = 0; $i -lt $Sizes.Count; $i++) {
        $s = $Sizes[$i]
        $w.Write([byte]($s % 256)); $w.Write([byte]($s % 256)); $w.Write([byte]0); $w.Write([byte]0)
        $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$imgs[$i].Length); $w.Write([uint32]$off)
        $off += $imgs[$i].Length
    }
    foreach ($b in $imgs) { $w.Write([byte[]]$b) }
    $w.Close()
    "wrote $name"
}

Write-Ico 'brand.ico' 'brand' ([string][char]0x015E)   # Ş
Write-Ico 'mode-on.ico' 'mode' ([string][char]0x015F)  # ş
Write-Ico 'mode-off.ico' 'mode' 's'
