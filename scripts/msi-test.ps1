# MSI install test (PLAN phase 4, WP14/WP15, 7.3). Run in an ELEVATED PowerShell on a clean Windows 10/11 user (or inside Windows Sandbox).
#   powershell -ExecutionPolicy Bypass -File scripts\msi-test.ps1 -Msi E:\office-target\p1\msi\hoshiya-0.1.0-x64.msi [-Evidence docs\phase4\evidence\openwith] [-Work <dir>] [-NoAlways] [-NoScreens]
# It installs the MSI, checks the registration and the "Open with" lists (SHAssocEnumHandlers = the shell's own enumeration), opens a docx,
# xlsx, pptx and pdf with the registered command line from paths with spaces and Cyrillic, checks the single-instance behaviour, drives the
# shell's "Open with > Always" for .docx, uninstalls, and diffs the registry before/after. Result lines: OPENWITH, REGISTRY, OPEN, SINGLE, ALWAYS, UNINSTALL.
# Reversible: the only per-user state it touches is HKCU ...\FileExts\.docx (UserChoice written by the SHELL when the dialog is driven);
# the dialog is driven only when .docx has no UserChoice yet, and that UserChoice is deleted again and checked. It never writes UserChoice
# itself. The shell's OpenWithList/MuiCache entries from opening files stay (reported as info). Screenshots stay in -Work: review them first.
# Elevation note: the editors are started from this elevated session, so they run elevated too (a medium-integrity launch from an elevated
# shell needs a helper such as `runas /trustlevel` or a scheduled task, which would change what is measured). Results are valid for the registry,
# the command lines and the windows; they do not show the medium-integrity behaviour of an ordinary double-click.
# Fixtures are tracked files only (docs/phase4/wp23/fixtures for the PDF).
# Exit code = number of failed checks (3 = not elevated, 2 = bad input).
param(
  [Parameter(Mandatory)][string]$Msi,
  [string]$Evidence = '',
  [string]$Work = (Join-Path $env:TEMP 'hoshiya-msi-test'),
  [string]$Fixtures = '',
  [switch]$NoAlways, [switch]$NoScreens, [switch]$KeepInstalled
)
$ErrorActionPreference = 'Stop'
# 5.1 leaves $PSScriptRoot empty in param defaults (seen with -File under RunAs), so resolve the script dir here
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Evidence) { $Evidence = Join-Path $here '..\docs\phase4\evidence\openwith' }
if (-not $Fixtures) { $Fixtures = Join-Path $here '..' }
$me = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $me.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { Write-Host 'NOT ELEVATED: start PowerShell as administrator (a per-machine MSI needs it).'; exit 3 }
if (-not (Test-Path -LiteralPath $Msi)) { Write-Host "no such msi: $Msi"; exit 2 }
$Msi = (Resolve-Path -LiteralPath $Msi).Path
New-Item -ItemType Directory -Force $Work, $Evidence | Out-Null
$Evidence = (Resolve-Path $Evidence).Path
Start-Transcript -Path (Join-Path $Evidence 'msi-test.log') -Force | Out-Null
$fails = 0
function Res($ok, $line) { if ($ok) { Write-Host "$line ok" } else { Write-Host "$line FAIL"; $script:fails++ } }

# ---- brand.toml names (the same regex reader as build-msi.ps1)
$toml = Get-Content (Join-Path $here '..\brand\brand.toml') -Raw -Encoding UTF8
function Sec($n) { [regex]::Match($toml, "(?ms)^\[$([regex]::Escape($n))\]\s*\r?\n(.*?)(?=^\[|\z)").Groups[1].Value }
function Key($s, $k) { [regex]::Match($s, "(?m)^$k\s*=\s*`"([^`"]*)`"").Groups[1].Value }
$prod = Sec 'product'; $productName = Key $prod 'name'; $productId = Key $prod 'id'
# the four checked types and the full registered lists (PLAN 7.2: no odp, no ppt)
$apps = @(
  @{ k = 'docs';   test = 'docx'; file = 'Буйруқ 1.docx';    second = 'Ikkinchi hujjat.docx'; src = 'docs\phase0\mobile-b\d-saved-desktop.docx';  ext = 'docx docm dotx dotm doc dot odt rtf txt md markdown html htm' },
  @{ k = 'sheets'; test = 'xlsx'; file = 'Hisobot 2026.xlsx'; second = 'Ikkinchi jadval.xlsx';  src = 'web\shell\templates\tabel.xlsx';               ext = 'xlsx xlsm xltx xltm xls xlt xlsb ods csv tsv' },
  @{ k = 'slides'; test = 'pptx'; file = 'Taqdimot 1.pptx';   second = 'Ikkinchi taqdimot.pptx'; src = 'web\shell\templates\taqdimot.pptx';          ext = 'pptx potx ppsx' },
  @{ k = 'pdf';    test = 'pdf';  file = 'Ҳисобот 1.pdf';    second = 'Ikkinchi hisobot.pdf';  src = 'docs\phase4\wp23\fixtures\namuna-hisobot.pdf'; ext = 'pdf' }
)
foreach ($a in $apps) { $s = Sec "apps.$($a.k)"; $a.bin = Key $s 'bin'; $a.aumid = Key $s 'aumid'; $a.data = (Key $s 'data_dir') -replace '/', '\'; $a.progid = "$($a.aumid).Document"; $a.ext = $a.ext -split ' '
  $a.name = [regex]::Match($s, 'names\s*=\s*\{[^}]*\ben\s*=\s*"([^"]*)"').Groups[1].Value }

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing @'
using System; using System.Collections.Generic; using System.IO; using System.Text; using System.Runtime.InteropServices;
public static class Shell {
  [ComImport, Guid("f04061ac-1659-4a3f-a954-775aa57fc083"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IAssocHandler {
    [PreserveSig] int GetName([MarshalAs(UnmanagedType.LPWStr)] out string n);
    [PreserveSig] int GetUIName([MarshalAs(UnmanagedType.LPWStr)] out string n);
    [PreserveSig] int GetIconLocation([MarshalAs(UnmanagedType.LPWStr)] out string p, out int i);
    [PreserveSig] int IsRecommended();
    [PreserveSig] int MakeDefault([MarshalAs(UnmanagedType.LPWStr)] string d);
    [PreserveSig] int Invoke(IntPtr o);
    [PreserveSig] int CreateInvoker(IntPtr o, out IntPtr i);
  }
  [ComImport, Guid("973810ae-9599-4b88-9e4d-6ee98c9552da"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IEnumAssocHandlers { [PreserveSig] int Next(uint c, [MarshalAs(UnmanagedType.Interface)] out IAssocHandler h, out uint f); }
  [DllImport("shell32.dll")] static extern int SHAssocEnumHandlers([MarshalAs(UnmanagedType.LPWStr)] string ext, int filter, out IEnumAssocHandlers e);
  // filter 0 = every handler, 1 = the recommended ones; returns "name|ui name|recommended"
  public static List<string> Handlers(string ext, int filter) {
    var r = new List<string>(); IEnumAssocHandlers e; if (SHAssocEnumHandlers(ext, filter, out e) != 0) return r;
    IAssocHandler h; uint f;
    while (e.Next(1, out h, out f) == 0 && f == 1) { string n, u; h.GetName(out n); h.GetUIName(out u); r.Add(n + "|" + u + "|" + (h.IsRecommended() == 0)); }
    return r;
  }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  // a .reg export (UTF-16) as a set of "key<TAB>value line"; every key header is an entry too, hex continuation lines are joined
  public static HashSet<string> RegSet(string path) {
    var s = new HashSet<string>(StringComparer.Ordinal); string key = ""; var sb = new StringBuilder(); bool cont = false;
    foreach (var raw in File.ReadLines(path, Encoding.Unicode)) {
      // binary values run over tens of thousands of "\" lines: string concat here was quadratic (hours on a 290 MB HKLM dump)
      var part = cont ? raw.TrimStart() : raw; if (!cont) sb.Clear();
      cont = part.EndsWith("\\"); sb.Append(part, 0, cont ? part.Length - 1 : part.Length);
      if (cont) continue;
      var l = sb.ToString();
      if (l.Length == 0 || l.StartsWith("Windows Registry Editor")) continue;
      if (l.StartsWith("[")) { key = l.Substring(1, l.Length - 2); s.Add(key + "\t"); } else s.Add(key + "\t" + l);
    }
    return s;
  }
  public static List<string> SymDiff(HashSet<string> a, HashSet<string> b) {
    var r = new List<string>(); foreach (var x in a) if (!b.Contains(x)) r.Add("- " + x); foreach (var x in b) if (!a.Contains(x)) r.Add("+ " + x); r.Sort(StringComparer.Ordinal); return r;
  }
  // b minus a, in C#: a PowerShell pipeline over a few million export lines takes far too long
  public static List<string> Minus(HashSet<string> b, HashSet<string> a) { var r = new List<string>(); foreach (var x in b) if (!a.Contains(x)) r.Add(x); return r; }
}
'@
[void][Shell]::SetProcessDPIAware()

function Shot($name) {
  if ($NoScreens) { return }
  $b = [Windows.Forms.SystemInformation]::VirtualScreen
  $bmp = New-Object Drawing.Bitmap $b.Width, $b.Height; $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [Drawing.Point]::Empty, $b.Size); $g.Dispose()
  # whole virtual screen (other windows of a work PC included): kept in $Work; look before copying any into the evidence folder
  $bmp.Save((Join-Path $Work "$name.png"), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}
Add-Type -AssemblyName System.Windows.Forms
function Snap($tag) {
  reg export HKLM\SOFTWARE "$Work\$tag-hklm.reg" /y | Out-Null; reg export HKCU\Software "$Work\$tag-hkcu.reg" /y | Out-Null
}
function UC($e) { $k = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.$e\UserChoice"; if (Test-Path $k) { (Get-ItemProperty $k).ProgId } else { '' } }
function Dflt($e) { (Get-ItemProperty "Registry::HKEY_CLASSES_ROOT\.$e" -ErrorAction SilentlyContinue).'(default)' }
function Procs($bin) { @(Get-Process -Name $bin -ErrorAction SilentlyContinue) }
function Launch($a, $path) {   # the registered command line, %1 replaced: what Explorer's "Open with" runs
  $cmd = (Get-ItemProperty "HKLM:\Software\Classes\$($a.progid)\shell\open\command").'(default)'
  $m = [regex]::Match($cmd, '^"([^"]+)"\s*(.*)$'); $args_ = $m.Groups[2].Value.Replace('"%1"', '"' + $path + '"')
  Start-Process -FilePath $m.Groups[1].Value -ArgumentList $args_ -PassThru
}
function WaitTitle($p, $needle, $sec = 40) {
  $t = [Diagnostics.Stopwatch]::StartNew(); while ($t.Elapsed.TotalSeconds -lt $sec) { $p.Refresh(); if ($p.HasExited) { return $null }; if ($p.MainWindowTitle -like "*$needle*") { return $p.MainWindowTitle }; Start-Sleep -Milliseconds 300 }; return $null
}

# ---- fixtures in a path with spaces and Cyrillic
$desk = Join-Path $Work 'Рабочий стол'; New-Item -ItemType Directory -Force $desk | Out-Null
foreach ($a in $apps) {
  $src = Join-Path $Fixtures $a.src; if (-not (Test-Path -LiteralPath $src)) { Write-Host "fixture missing: $src"; exit 2 }
  Copy-Item -LiteralPath $src -Destination (Join-Path $desk $a.file) -Force; Copy-Item -LiteralPath $src -Destination (Join-Path $desk $a.second) -Force
}

# ---- before
Write-Host "== before ($(Get-Date -Format s))"
$ucBefore = @{}; foreach ($e in 'docx', 'xlsx', 'pptx', 'pdf') { $ucBefore[$e] = "$(UC $e)|$(Dflt $e)" }
$fileExtsDocx = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.docx'
if (Test-Path "Registry::HKEY_CURRENT_USER\$($fileExtsDocx.Substring(5))") { reg export $fileExtsDocx "$Work\fileexts-docx-before.reg" /y | Out-Null }   # absent on a fresh profile; reg's stderr would be terminating under 'Stop'
$existing = @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall' | ForEach-Object { Get-ChildItem $_ -ErrorAction SilentlyContinue } | Where-Object { (Get-ItemProperty $_.PSPath).DisplayName -eq $productName })
if ($existing) { Write-Host "a $productName package is already installed; uninstall it first"; Stop-Transcript | Out-Null; exit 2 }
Snap 'A'

# ---- install
Write-Host '== install'
$il = Join-Path $Work 'install.log'
$rc = (Start-Process msiexec.exe -ArgumentList "/i `"$Msi`" /qn /norestart /l*v `"$il`"" -Wait -PassThru).ExitCode
Res ($rc -eq 0 -or $rc -eq 3010) "INSTALL exit $rc"
$productCode = [regex]::Match((Get-Content $il -Raw), 'Property\(S\): ProductCode = (\{[0-9A-F-]+\})').Groups[1].Value
$inst = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\$($apps[0].bin).exe" -ErrorAction SilentlyContinue).'(default)'
$instDir = if ($inst) { Split-Path $inst } else { "$env:ProgramFiles\$productName" }
Snap 'B'

$testsDone = $false
try {
# ---- registration
Write-Host '== registry'
$regOk = $true; $nKeys = 0
foreach ($a in $apps) {
  $cap = (Get-ItemProperty "HKLM:\Software\$($a.data)\Capabilities" -ErrorAction SilentlyContinue)
  if (-not $cap -or $cap.ApplicationName -ne $a.name) { Write-Host "  no Capabilities for $($a.k)"; $regOk = $false }
  if ((Get-ItemProperty 'HKLM:\Software\RegisteredApplications' -ErrorAction SilentlyContinue).($a.name) -ne "Software\$($a.data)\Capabilities") { Write-Host "  RegisteredApplications missing: $($a.name)"; $regOk = $false }
  $cmd = (Get-ItemProperty "HKLM:\Software\Classes\$($a.progid)\shell\open\command" -ErrorAction SilentlyContinue).'(default)'
  if ($cmd -ne "`"$instDir\$($a.bin).exe`" `"%1`"") { Write-Host "  bad ProgID command for $($a.progid): $cmd"; $regOk = $false }
  $app = "HKLM:\Software\Classes\Applications\$($a.bin).exe"
  $st = (Get-Item "$app\SupportedTypes" -ErrorAction SilentlyContinue).Property
  foreach ($e in $a.ext) {
    $nKeys++
    $ow = (Get-Item "HKLM:\Software\Classes\.$e\OpenWithProgids" -ErrorAction SilentlyContinue).Property
    if ($ow -notcontains $a.progid) { Write-Host "  .$e OpenWithProgids lacks $($a.progid)"; $regOk = $false }
    if ($st -notcontains ".$e") { Write-Host "  SupportedTypes lacks .$e"; $regOk = $false }
    if ((Get-ItemProperty "HKLM:\Software\$($a.data)\Capabilities\FileAssociations" -ErrorAction SilentlyContinue).".$e" -ne $a.progid) { Write-Host "  FileAssociations lacks .$e"; $regOk = $false }
  }
  if (-not (Test-Path (Join-Path $instDir "$($a.bin).exe"))) { Write-Host "  exe missing: $($a.bin)"; $regOk = $false }
}
foreach ($no in 'odp', 'ppt') { if ((Get-Item "HKLM:\Software\Classes\.$no\OpenWithProgids" -ErrorAction SilentlyContinue).Property -match 'Hoshiya|\.Document') { Write-Host "  .$no is registered but must not be"; $regOk = $false } }
foreach ($e in 'docx', 'xlsx', 'pptx', 'pdf') { if ("$(UC $e)|$(Dflt $e)" -ne $ucBefore[$e]) { Write-Host "  default of .$e CHANGED by the install"; $regOk = $false } }
$menu = Get-ChildItem "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\$productName" -Filter *.lnk -ErrorAction SilentlyContinue
if ($menu.Count -ne 5) { Write-Host "  Start menu shortcuts: $($menu.Count) (want 5)"; $regOk = $false }
Res $regOk "REGISTRY $nKeys extension registrations, 4 ProgIDs, 4 Capabilities, 5 shortcuts, no default changed,"
foreach ($f in 'dist\shell\index.html', 'dist\docs-web', 'dictionaries\en_US.dic', 'fonts\NotoSans-Regular-uz.ttf', 'NOTICE', 'brand.toml', 'convert\docs-cli.exe', 'licenses\logo-LICENSE.txt') { if (-not (Test-Path (Join-Path $instDir $f))) { Write-Host "  file missing: $f"; $fails++ } }

# ---- Open with (the shell's own enumeration), and the real dialogs for the screenshots
Write-Host '== open with'
foreach ($a in $apps) {
  $all = [Shell]::Handlers(".$($a.test)", 0).Count
  $list = [Shell]::Handlers(".$($a.test)", 1)   # filter 1 = the recommended handlers, what the right-click Open with menu lists
  $mine = @($list | Where-Object { $_ -like "*\$($a.bin).exe|*" -or $_ -like "*|$($a.name)|*" })
  Write-Host "  .$($a.test): $($list.Count) recommended ($all in all); ours: $($mine -join '; ')"
  Res ($mine.Count -ge 1) "OPENWITH $($a.test)"
}
if (-not $NoScreens) {
  foreach ($a in $apps) {   # the classic Open with dialog; the explorer menu is shot below
    $f = Join-Path $desk $a.file
    $p = Start-Process rundll32.exe -ArgumentList "shell32.dll,OpenAs_RunDLL `"$f`"" -PassThru
    Start-Sleep -Seconds 4; Shot "openwith-dialog-$($a.test)"
    Get-Process OpenWith -ErrorAction SilentlyContinue | Stop-Process -Force; Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  }
  # the real Explorer context menu: select the file in an Explorer window, Shift+F10, "Open with" (H)
  try {
    $ws = New-Object -ComObject Shell.Application
    foreach ($a in @($apps[0])) {
      Start-Process explorer.exe -ArgumentList "/select,`"$(Join-Path $desk $a.file)`""; Start-Sleep -Seconds 4
      $sh = New-Object -ComObject WScript.Shell; if (-not $sh.AppActivate('Рабочий стол')) { throw 'Explorer window not found (AppActivate); keystrokes would go elsewhere' }; Start-Sleep -Milliseconds 500
      $sh.SendKeys('+{F10}'); Start-Sleep -Milliseconds 800; Shot "explorer-menu-$($a.test)"
      $sh.SendKeys('h'); Start-Sleep -Milliseconds 800; $sh.SendKeys('{RIGHT}'); Start-Sleep -Milliseconds 800; Shot "explorer-openwith-submenu-$($a.test)"
      $sh.SendKeys('{ESC}{ESC}{ESC}')
      foreach ($w in $ws.Windows()) { if ($w.LocationURL -and ([uri]$w.LocationURL).LocalPath -ieq $desk) { $w.Quit() } }
    }
  } catch { Write-Host "  explorer menu screenshot not taken: $_" }
}

# ---- open, with the registered command line; paths with spaces and Cyrillic
Write-Host '== open'
$first = @{}; $nOpen = 0
foreach ($a in $apps) {
  $f = Join-Path $desk $a.file; $p = Launch $a $f; $first[$a.k] = $p
  $needle = [IO.Path]::GetFileNameWithoutExtension($a.file)
  $t = WaitTitle $p $needle 45
  if (-not $t -and $a.k -eq 'slides') { Start-Sleep 5; $p.Refresh(); $t = if (-not $p.HasExited -and $p.MainWindowHandle) { "(window up, title [$($p.MainWindowTitle)])" } }   # Slides' title carries the app name only
  Write-Host "  $($a.bin): title [$($p.MainWindowTitle)]"
  if ($t) { $nOpen++ } else { Write-Host "  $($a.bin) FAILED to show $needle" }
}
Shot 'open-4'
Res ($nOpen -eq 4) "OPEN spaces+cyrillic $nOpen/4"

# ---- single instance
Write-Host '== single instance'
$single = @()
foreach ($a in $apps) {
  $f2 = Join-Path $desk $a.second; $p2 = Launch $a $f2; $needle2 = [IO.Path]::GetFileNameWithoutExtension($a.second)
  if ($a.k -eq 'pdf') {
    $exited = $p2.WaitForExit(20000); Start-Sleep 2; $one = (Procs $a.bin).Count -eq 1
    Write-Host "  pdf: second launch exited=$exited, processes=$((Procs $a.bin).Count), first title [$($first[$a.k].MainWindowTitle)]"
    Shot 'single-pdf'; if ($exited -and $one) { $single += 'pdf tab ok' } else { $single += 'pdf tab FAIL'; $fails++ }
  } else {
    $t2 = WaitTitle $p2 $needle2 45; $n = (Procs $a.bin).Count; $first[$a.k].Refresh()
    Write-Host "  $($a.k): processes=$n, second title [$($p2.MainWindowTitle)], first still [$($first[$a.k].MainWindowTitle)]"
    $ok = ($n -eq 2) -and ($t2 -or $a.k -eq 'slides') -and -not $first[$a.k].HasExited
    if ($ok) { $single += "$($a.k) new-window ok" } else { $single += "$($a.k) new-window FAIL"; $fails++ }
  }
}
Shot 'single-all'
Write-Host "SINGLE $($single -join ', ')"
foreach ($a in $apps) { Procs $a.bin | Stop-Process -Force -ErrorAction SilentlyContinue }
Start-Sleep 1

# ---- "Always": the SHELL writes UserChoice; this only drives its dialog (UI Automation) and then checks a plain double-click
Write-Host '== always'
$alwaysLine = 'ALWAYS docx not run (-NoAlways)'
# an existing .docx UserChoice is never driven: the shell's hashed, deny-ACL'd key cannot be put back by a script
if (-not $NoAlways -and $ucBefore['docx'] -notlike '|*') { $NoAlways = $true; $alwaysLine = 'ALWAYS docx not run (.docx already has a UserChoice; left untouched)' }
if (-not $NoAlways) {
  Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
  $docx = Join-Path $desk $apps[0].file; $ac = $apps[0]
  $dlg = Start-Process rundll32.exe -ArgumentList "shell32.dll,OpenAs_RunDLL `"$docx`"" -PassThru; Start-Sleep -Seconds 4
  $root = [Windows.Automation.AutomationElement]::RootElement
  $win = $null; foreach ($i in 1..10) { $win = $root.FindAll('Children', [Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.ClassName -match 'OpenWith|ApplicationFrameWindow|Shell_Dialog' -or $_.Current.Name -match 'Open with|How do you want|Как вы хотите|Открыть с помощью' } | Select-Object -First 1; if ($win) { break }; Start-Sleep 1 }
  $done = $false
  if ($win) {
    function Find($name, $type) { $c = New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty), $name; $win.FindFirst('Descendants', $c) }
    function FindLike($pat) { $win.FindAll('Descendants', [Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -match $pat } | Select-Object -First 1 }
    $item = FindLike ([regex]::Escape($ac.name))
    if (-not $item) { $more = FindLike 'More apps|Ещё приложения|Другие приложения'; if ($more) { try { $more.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() } catch {}; Start-Sleep 2; $item = FindLike ([regex]::Escape($ac.name)) } }
    if ($item) {
      try { $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select() } catch { try { $item.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() } catch {} }
      Start-Sleep 1
      $chk = FindLike 'Always use|Всегда использовать'
      if ($chk) { try { $tp = $chk.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern); if ($tp.Current.ToggleState -ne 'On') { $tp.Toggle() } } catch {} }
      Start-Sleep 1; Shot 'always-dialog'
      $ok = FindLike '^(OK|ОК)$'
      if ($ok) { try { $ok.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); $done = $true } catch {} }
    }
  }
  Start-Sleep 6
  $uc = UC 'docx'
  if ($done -and $uc -eq $ac.progid) {
    # a plain double-click is ShellExecute "open" on the file
    $before = (Procs $ac.bin).Count; Start-Process -FilePath $docx; $t = $null
    foreach ($i in 1..40) { Start-Sleep -Milliseconds 500; $pp = Procs $ac.bin; if ($pp.Count -gt $before) { $t = ($pp | Select-Object -First 1).MainWindowTitle; break } }
    Shot 'always-doubleclick'
    $alwaysLine = if ($t -ne $null) { 'ALWAYS docx ok' } else { 'ALWAYS docx FAIL (UserChoice set, but the double-click did not start the app)' }
  } else { $alwaysLine = "ALWAYS docx NOT DRIVEN (dialog automation found: window=$([bool]$win) item=$([bool]$item) done=$done; UserChoice=[$uc]) - do it by hand: right-click a .docx > Open with > $($ac.name) > Always" }
  Get-Process OpenWith -ErrorAction SilentlyContinue | Stop-Process -Force; Stop-Process -Id $dlg.Id -Force -ErrorAction SilentlyContinue
  foreach ($a in $apps) { Procs $a.bin | Stop-Process -Force -ErrorAction SilentlyContinue }
  # revert: the UserChoice the shell wrote (the state before was "no UserChoice", checked above). reg.exe, not Remove-Item: the key
  # carries a deny-SetValue ACE that makes PowerShell's writable open fail silently. Our OpenWithList entries are shell artefacts (info).
  $ucKey = "$fileExtsDocx\UserChoice"
  if (UC 'docx') { try { & reg.exe delete $ucKey /f 2>&1 | Out-Null } catch {} }   # 5.1: native stderr under 'Stop' throws; the check below decides
  Res ((UC 'docx') -eq '') "ALWAYS revert (.docx UserChoice removed again; if FAIL: reg delete `"$ucKey`" /f)"
}
Write-Host $alwaysLine; if ($alwaysLine -match 'FAIL') { $fails++ }
$testsDone = $true
} finally {   # a failed check or a thrown error must not leave the package installed
  if (-not $testsDone -and -not $KeepInstalled) {
    Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($instDir, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Process msiexec.exe -ArgumentList "/x `"$Msi`" /qn /norestart /l*v `"$Work\uninstall-after-error.log`"" -Wait | Out-Null
  }
}

# ---- uninstall
Write-Host '== uninstall'
Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($instDir, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue
if (-not $KeepInstalled) {
  $urc = (Start-Process msiexec.exe -ArgumentList "/x `"$Msi`" /qn /norestart /l*v `"$Work\uninstall.log`"" -Wait -PassThru).ExitCode
  Res ($urc -eq 0 -or $urc -eq 3010) "UNINSTALL exit $urc"
  Snap 'C'
  $left = @(); if (Test-Path $instDir) { $left += "folder $instDir" }; if (Test-Path "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\$productName") { $left += 'start menu folder' }
  $da = [Shell]::RegSet("$Work\A-hklm.reg"); $dc = [Shell]::RegSet("$Work\C-hklm.reg"); $dd = [Shell]::SymDiff($da, $dc)
  $ua = [Shell]::RegSet("$Work\A-hkcu.reg"); $uc2 = [Shell]::RegSet("$Work\C-hkcu.reg"); $ud = [Shell]::SymDiff($ua, $uc2)
  # ours = mentions the product, its ProgIDs / exes / codes, or is one of the extension keys we wrote to
  $exts = ($apps | ForEach-Object { $_.ext } | ForEach-Object { [regex]::Escape($_) }) -join '|'
  $mine = "$productName|$productId|" + $(if ($productCode) { "$([regex]::Escape($productCode))|" }) + (($apps | ForEach-Object { [regex]::Escape($_.progid) + '|' + [regex]::Escape($_.bin) }) -join '|') + "|\\\.($exts)(\\OpenWithProgids)?\t"
  # blocking: a line the INSTALL added (A -> B, ours) that is still there after uninstall. The shell's own per-user artefacts of opening
  # files (OpenWithList, MuiCache, AppCompat store, association toasts) appear only after B and are reported as info, not counted.
  $hb = [Shell]::RegSet("$Work\B-hklm.reg"); $ub = [Shell]::RegSet("$Work\B-hkcu.reg")
  $added = @(@([Shell]::Minus($hb, $da)) + @([Shell]::Minus($ub, $ua)) | Where-Object { $_ -match $mine })
  $residue = @($added | Where-Object { $dc.Contains($_) -or $uc2.Contains($_) })
  $ours = @($dd + $ud | Where-Object { $_ -match $mine })
  $other = @($dd + $ud | Where-Object { $_ -notmatch $mine })
  # the A/C diffs stay in $Work: they hold every unrelated change on this PC during the run (MRU file names); only counts go to evidence
  Set-Content (Join-Path $Evidence 'uninstall-residue.txt') $residue -Encoding UTF8; Set-Content (Join-Path $Work 'uninstall-diff-ours-incl-shell.txt') $ours -Encoding UTF8; Set-Content (Join-Path $Work 'uninstall-diff-other-machine-noise.txt') $other -Encoding UTF8
  Res (($residue.Count + $left.Count) -eq 0) "UNINSTALL residue $($residue.Count) of $($added.Count) installed registry lines (files left: $($left.Count); info: product-related shell artefacts $($ours.Count - $residue.Count), other changes on the machine $($other.Count), both in $Work)"
  $residue | Select-Object -First 30 | ForEach-Object { Write-Host "  left: $_" }
  Set-Content (Join-Path $Evidence 'install-added.txt') $added -Encoding UTF8; Write-Host "  install added $($added.Count) registry lines (install-added.txt)"
  Remove-Item "$Work\*-hkcu.reg" -Force -ErrorAction SilentlyContinue   # whole-HKCU dumps: personal data, not kept
}
Write-Host "== done: $fails failed check(s)"
Stop-Transcript | Out-Null
exit $fails
