cd E:/office-target/xlsx-bench
C=E:/office-tools/releases/win/gridcraft-0.7.0-windows-x64-portable/gridcraft-0.7.0-windows-x64-portable/gridcraft-cli.exe
M="E:/office-tools/venv-bench/Scripts/python.exe -I C:/Users/uge226/Desktop/office/spikes/xlsx-bench/measure.py"
for n in 40 80 120; do
  $M info$n -- $C info w$n.xlsx
  $M conv$n -- $C convert w$n.xlsx w${n}_out.xlsx
  if [ -f w${n}_out.xlsx ]; then ls -l w${n}_out.xlsx; $M reopen$n -- $C info w${n}_out.xlsx; fi
done
echo done > sizes.done
