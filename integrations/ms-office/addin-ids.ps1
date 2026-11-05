# The add-in's fixed identity, shared by register.ps1, unregister.ps1 and the tests. Must match src/Connect.cs.
# The product name is not here: it comes from brand/brand.toml via brand-name.txt.
$Clsid = '{B1F8F147-98CA-4DF8-8C5D-E2E2D4E4E3A8}'
$ProgId = 'UzTranslit.OfficeAddin'
$ClassName = 'UzTranslit.Office.Connect'
$FriendlySuffix = 'Uzbek transliteration'
$Description = 'Uzbek Latin <-> Cyrillic transliteration (1995 and 2026 Latin alphabets)'
# Every app register.ps1 may write to; unregister removes all of them. Outlook is listed so a future registration is cleaned up too.
$AllApps = @('Word', 'Excel', 'PowerPoint', 'Outlook')
# MSI UpgradeCode of the add-in package (installer/addin.wxs): FROZEN, generated once (2026-11-05). Never change it, or a new
# MSI no longer replaces the installed one. Kept here, not in brand/brand.toml.
$UpgradeCode = '{6C0E5B7A-3D41-4F6B-9A52-8E17C4D0B2F9}'
