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
