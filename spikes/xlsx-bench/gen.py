"""Synthetic workload generator. Deterministic (seed 42).
Usage: gen.py OUT.xlsx --sheets 30 --rows 5000 [--cached] [--json EXPECT.json]
Default: openpyxl write_only (formulas have NO cached values).
--cached: xlsxwriter (BSD, test generator only) so formulas carry computed cached values.
Sheets S01..Snn: A id, B date, C amount, D qty, E text(Cyrillic+Latin), F cat,
  G =IF(C>500,C*D,0)  H =VLOOKUP(F,Lookup!$A$2:$B$21,2,FALSE)  I =C+D (S01) / =S(k-1)!C+D (cross-sheet)
Sheet Lookup: 20 rows K00..K19 -> factor. Sheet Summary: per sheet SUM, SUMIFS, XLOOKUP, COUNTIF.
"""
import argparse, random, json, datetime
ap = argparse.ArgumentParser()
ap.add_argument("out"); ap.add_argument("--sheets", type=int, default=30)
ap.add_argument("--rows", type=int, default=5000); ap.add_argument("--cached", action="store_true")
ap.add_argument("--json"); a = ap.parse_args()
rnd = random.Random(42)
WORDS = ["Приход","Расход","Оплата","Склад","Товар","Order","Invoice","Клиент","Supplier","Возврат","Delivery","Акт"]
base = datetime.date(2020,1,1)
lookup = [(f"K{i:02d}", round(1+i*0.05,2)) for i in range(20)]
sheets = []; prevC = prevD = None
exp = {"sheets":{}, "lookup":dict(lookup), "rows":a.rows}
for s in range(a.sheets):
    rows = []; Cs=[]; Ds=[]; sumG=0.0; sumC_k05=0.0; cnt_k05=0; first_k05=None
    for r in range(a.rows):
        amt = round(rnd.uniform(1,1000),2); qty = rnd.randint(1,50)
        cat = f"K{rnd.randint(0,19):02d}"; d = base+datetime.timedelta(days=rnd.randint(0,2000))
        txt = f"{rnd.choice(WORDS)} {rnd.choice(WORDS)} №{rnd.randint(1000,99999)}"
        g = amt*qty if amt>500 else 0
        h = dict(lookup)[cat]
        i = (prevC[r]+prevD[r]) if prevC else amt+qty
        rows.append((r+1,d,amt,qty,txt,cat,g,h,i)); Cs.append(amt); Ds.append(qty)
        sumG += g
        if cat=="K05": sumC_k05+=amt; cnt_k05+=1; first_k05 = first_k05 or (r+1)
    exp["sheets"][f"S{s+1:02d}"] = dict(sumG=sumG, sumC_k05=sumC_k05, cnt_k05=cnt_k05, last=rows[-1][1:] and [str(rows[-1][1]),*rows[-1][2:]], row2=[str(rows[0][1]),*rows[0][2:]])
    sheets.append(rows); prevC, prevD = Cs, Ds
N = a.rows+1
def frm(s, r):  # formulas for row r (1-based sheet row), sheet index s
    i = f"=S{s:02d}!C{r}+D{r}" if s>1 else f"=C{r}+D{r}"
    return [f"=IF(C{r}>500,C{r}*D{r},0)", f"=VLOOKUP(F{r},Lookup!$A$2:$B$21,2,FALSE)", i]
def sumfs(s): 
    n=f"S{s:02d}"
    return [f"=SUM({n}!G2:G{N})", f'=SUMIFS({n}!C2:C{N},{n}!F2:F{N},"K05")', f'=XLOOKUP("K05",{n}!F2:F{N},{n}!A2:A{N})', f'=COUNTIF({n}!F2:F{N},"K05")']
if not a.cached:
    from openpyxl import Workbook
    from openpyxl.cell import WriteOnlyCell
    wb = Workbook(write_only=True)
    ws = wb.create_sheet("Lookup"); ws.append(["Key","Factor"]); [ws.append(list(x)) for x in lookup]
    for s,rows in enumerate(sheets,1):
        ws = wb.create_sheet(f"S{s:02d}"); ws.append(["ID","Date","Amount","Qty","Text","Cat","Big","Factor","Link"])
        for r,row in enumerate(rows,2):
            c = WriteOnlyCell(ws, value=row[1]); c.number_format="yyyy-mm-dd"
            ws.append([row[0],c,row[2],row[3],row[4],row[5],*frm(s,r)])
    ws = wb.create_sheet("Summary",0); ws.append(["Sheet","SumG","SumIfsK05","FirstK05","CountK05"])
    for s in range(1,a.sheets+1): ws.append([f"S{s:02d}",*sumfs(s)])
    wb.save(a.out)
else:
    import xlsxwriter
    wb = xlsxwriter.Workbook(a.out, {"constant_memory": False})
    df = wb.add_format({"num_format":"yyyy-mm-dd"})
    sm = wb.add_worksheet("Summary"); lk = wb.add_worksheet("Lookup")
    lk.write_row(0,0,["Key","Factor"]); [lk.write_row(i+1,0,list(x)) for i,x in enumerate(lookup)]
    sm.write_row(0,0,["Sheet","SumG","SumIfsK05","FirstK05","CountK05"])
    for s,rows in enumerate(sheets,1):
        ws = wb.add_worksheet(f"S{s:02d}"); ws.write_row(0,0,["ID","Date","Amount","Qty","Text","Cat","Big","Factor","Link"])
        for r,row in enumerate(rows,2):
            ws.write_number(r-1,0,row[0]); ws.write_datetime(r-1,1,datetime.datetime.combine(row[1],datetime.time()),df)
            ws.write_number(r-1,2,row[2]); ws.write_number(r-1,3,row[3]); ws.write_string(r-1,4,row[4]); ws.write_string(r-1,5,row[5])
            f = frm(s,r)
            for j in range(3): ws.write_formula(r-1,6+j,f[j],None,row[6+j])
    for s in range(1,a.sheets+1):
        e = exp["sheets"][f"S{s:02d}"]; f = sumfs(s)
        first = next(i for i,row in enumerate(sheets[s-1],1) if row[5]=="K05")
        sm.write(s,0,f"S{s:02d}")
        for j,v in enumerate([e["sumG"],e["sumC_k05"],first,e["cnt_k05"]]): sm.write_formula(s,1+j,f[j],None,v)
    wb.close()
if a.json: json.dump(exp, open(a.json,"w"), default=str)
