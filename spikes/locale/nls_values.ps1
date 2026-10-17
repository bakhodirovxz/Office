# Windows NLS number/date patterns for the three target locales (read-only, no culture switch).
foreach ($n in 'uz-Latn-UZ','uz-Cyrl-UZ','ru-RU','en-US') {
  try {
    $c = [Globalization.CultureInfo]::GetCultureInfo($n); $nf = $c.NumberFormat; $dt = $c.DateTimeFormat
    $ci = [Globalization.CultureInfo]::new($n,$false)
    $g = [string]::Join(',', ($nf.NumberGroupSizes))
    "{0}`tdec=[{1}] grp=[{2}] (U+{3:X4}) grpSizes={4} list=[{5}] pct=[{6}] short=[{7}] long=[{8}]`tN2={9}`tdate={10}`tmonth10={11}`tgenMonth10={12}`tday={13}" -f $n,$nf.NumberDecimalSeparator,$nf.NumberGroupSeparator,[int][char]$nf.NumberGroupSeparator[0],$g,$c.TextInfo.ListSeparator,$nf.PercentPositivePattern,$dt.ShortDatePattern,$dt.LongDatePattern,(3500000.5).ToString('N2',$c),(Get-Date '2026-10-12').ToString('d MMMM yyyy',$c),$dt.GetMonthName(10),$dt.MonthGenitiveNames[9],$dt.GetDayName([DayOfWeek]::Monday)
  } catch { "$n`tERR $($_.Exception.Message)" }
}
