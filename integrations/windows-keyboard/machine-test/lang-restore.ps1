# Restores the user's language list from the lines run.sh saved before the test ("<tag> <tip>,<tip>,...", one per
# language, in order). Safety net only: KbdUninstallForUser has already removed the keyboard.
param([string]$Saved)
$l = $null
foreach ($line in (Get-Content $Saved -Encoding UTF8 | Where-Object { $_.Trim() })) {
    $tag, $tips = $line.Trim().Split(' ', 2)
    if ($null -eq $l) { $l = New-WinUserLanguageList $tag } else { $l.Add($tag) }
    $e = $l[$l.Count - 1]
    $e.InputMethodTips.Clear()
    foreach ($t in ("$tips".Split(',') | Where-Object { $_ })) { $e.InputMethodTips.Add($t) }
}
Set-WinUserLanguageList $l -Force
Get-WinUserLanguageList | ForEach-Object { $_.LanguageTag + ' ' + ($_.InputMethodTips -join ',') }
