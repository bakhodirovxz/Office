"""Variant of spikes/xlsx-bench/gen.py (same seed/data). --cols 4: A-D numbers only; --cols 6: A-F (no formulas)."""
import sys, random, datetime
from openpyxl import Workbook
from openpyxl.cell import WriteOnlyCell
out, ncols, nsheets, nrows = sys.argv[1], int(sys.argv[2]), 30, 4800
rnd = random.Random(42)
WORDS = ["Приход","Расход","Оплата","Склад","Товар","Order","Invoice","Клиент","Supplier","Возврат","Delivery","Акт"]
base = datetime.date(2020,1,1)
wb = Workbook(write_only=True)
ws = wb.create_sheet("Lookup"); ws.append(["Key","Factor"]); [ws.append([f"K{i:02d}", round(1+i*0.05,2)]) for i in range(20)]
for s in range(1, nsheets+1):
    ws = wb.create_sheet(f"S{s:02d}"); ws.append(["ID","Date","Amount","Qty","Text","Cat","Big","Factor","Link"][:ncols])
    for r in range(nrows):
        amt = round(rnd.uniform(1,1000),2); qty = rnd.randint(1,50)
        cat = f"K{rnd.randint(0,19):02d}"; d = base+datetime.timedelta(days=rnd.randint(0,2000))
        txt = f"{rnd.choice(WORDS)} {rnd.choice(WORDS)} №{rnd.randint(1000,99999)}"
        c = WriteOnlyCell(ws, value=d); c.number_format="yyyy-mm-dd"
        ws.append([r+1, c, amt, qty, txt, cat][:ncols])
ws = wb.create_sheet("Summary",0); ws.append(["Sheet"])
wb.save(out)
