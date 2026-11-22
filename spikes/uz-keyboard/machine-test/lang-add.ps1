$l = Get-WinUserLanguageList
$l.Add('uz-Latn-UZ')
$u = $l | Where-Object LanguageTag -like 'uz-Latn*'
$u.InputMethodTips.Clear()
$u.InputMethodTips.Add('0443:{71A6784F-8ECA-4FC0-92C0-0B1756DE11CB}{F617E325-D178-40A9-9070-BF4B61B266B4}')
Set-WinUserLanguageList $l -Force
Get-WinUserLanguageList | ForEach-Object { $_.LanguageTag + ' ' + ($_.InputMethodTips -join ',') }
