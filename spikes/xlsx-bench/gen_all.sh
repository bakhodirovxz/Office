cd E:/office-target/xlsx-bench
P=E:/office-tools/venv-bench/Scripts/python.exe; G=C:/Users/uge226/Desktop/office/spikes/xlsx-bench/gen.py
$P -I $G w10.xlsx --sheets 30 --rows 4800 --json w10.json
$P -I $G c10.xlsx --sheets 30 --rows 3000 --cached --json c10.json
$P -I $G w40.xlsx --sheets 30 --rows 19200
$P -I $G w80.xlsx --sheets 30 --rows 38400
$P -I $G w120.xlsx --sheets 30 --rows 57600
echo done > gen.done
