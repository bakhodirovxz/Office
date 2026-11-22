# Restores the list read before the test: en-US 0409:00000409, ru 0419:00000419.
$l = New-WinUserLanguageList 'en-US'
$l[0].InputMethodTips.Clear()
$l[0].InputMethodTips.Add('0409:00000409')
$l.Add('ru')
$r = $l | Where-Object LanguageTag -eq 'ru'
$r.InputMethodTips.Clear()
$r.InputMethodTips.Add('0419:00000419')
Set-WinUserLanguageList $l -Force
Get-WinUserLanguageList | ForEach-Object { $_.LanguageTag + ' ' + ($_.InputMethodTips -join ',') }
# Windows leaves these two per-user keys for the removed profile (machine runs 2026-11-22); remove them.
foreach ($k in 'HKCU:\Software\Microsoft\CTF\SortOrder\AssemblyItem\0x00000443', 'HKCU:\Software\Microsoft\CTF\TIP\{71A6784F-8ECA-4FC0-92C0-0B1756DE11CB}') {
    if (Test-Path $k) { Remove-Item -Recurse -Force $k; "removed $k" }
}
