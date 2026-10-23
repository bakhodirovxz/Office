# WP18: dumps the UI Automation tree (what Narrator reads) of an egui/AccessKit app window, and optionally walks Tab.
#   powershell -File scripts/uia-dump.ps1 -Exe <app.exe> -File <doc> [-Invoke "Continue without restoring"] [-Tabs 30] [-Out dump.txt]
# Needs an unlocked interactive desktop for -Tabs (SendKeys goes to the foreground window). The tree dump works without it.
# Tree-only runs of the four release apps: docs/phase4/evidence/a11y/uia-*-before.txt
param([string]$Exe,[string]$File="",[int]$Wait=10,[string]$Out="",[int]$Tabs=0,[string]$Invoke="")
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
$p = if ($File) { Start-Process $Exe -ArgumentList $File -PassThru } else { Start-Process $Exe -PassThru }
try {
  Start-Sleep -Seconds $Wait
  $root=[System.Windows.Automation.AutomationElement]::RootElement
  $w=$root.FindAll([System.Windows.Automation.TreeScope]::Children,(New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$p.Id))) | Where-Object { $_.Current.Name } | Select-Object -First 1
  try { $w.SetFocus() } catch {}; Start-Sleep 2
  $w=[System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$w.Current.NativeWindowHandle)
  if ($Invoke) { $w2=[System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$w.Current.NativeWindowHandle); $b=$w2.FindFirst([System.Windows.Automation.TreeScope]::Descendants,(New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$Invoke))); if ($b) { $b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); Start-Sleep 5 } }
  $walker=[System.Windows.Automation.TreeWalker]::RawViewWalker
  $lines=New-Object System.Collections.ArrayList
  function Walk($e,$d){ $k=$walker.GetFirstChild($e); while($k){ $c=$k.Current; $r=$c.BoundingRectangle
    [void]$lines.Add(("{0}{1}`t{2}`tfocusable={3}`tenabled={4}`t{5:N0},{6:N0},{7:N0}x{8:N0}" -f (' '*$d),$c.ControlType.ProgrammaticName.Replace('ControlType.',''),$c.Name,$c.IsKeyboardFocusable,$c.IsEnabled,$r.X,$r.Y,$r.Width,$r.Height)); Walk $k ($d+1); $k=$walker.GetNextSibling($k) } }
  Walk $w 0
  if ($Tabs -gt 0) {
    [void]$lines.Add("--- tab walk")
    for($i=1;$i -le $Tabs;$i++){ [System.Windows.Forms.SendKeys]::SendWait("{TAB}"); Start-Sleep -Milliseconds 350
      $f=[System.Windows.Automation.AutomationElement]::FocusedElement; $c=$f.Current
      [void]$lines.Add(("tab{0}`t{1}`t[{2}]`t{3}" -f $i,$c.ControlType.ProgrammaticName.Replace('ControlType.',''),$c.Name,$c.ProcessId)) }
  }
  if ($Out) { $lines | Set-Content -Encoding utf8 $Out } else { $lines }
} finally { Stop-Process -Id $p.Id -Force }


