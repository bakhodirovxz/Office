import json, random
r0 = 19202; n = 10000; rnd = random.Random(7); vals=[]; sg=0.0
for i in range(n):
    r = r0+i; amt = round(rnd.uniform(1,1000),2); qty = rnd.randint(1,50); cat=f"K{rnd.randint(0,19):02d}"
    vals.append([r-1, 44000+rnd.randint(0,2000), amt, qty, f"Добавлено Added №{r}", cat, f"=IF(C{r}>500,C{r}*D{r},0)", f"=VLOOKUP(F{r},Lookup!$A$2:$B$21,2,FALSE)", f"=C{r}+D{r}"])
    if amt>500: sg += amt*qty
open("E:/office-target/xlsx-bench/append.jsonl","w").write(
 json.dumps({"id":"range.setValues","params":{"range":f"S01!A{r0}","values":vals}})+"\n"+
 json.dumps({"id":"cell.set","params":{"cell":"Summary!H1","value":f"=SUM(S01!G2:G{r0+n-1})"}})+"\n"+
 json.dumps({"id":"cell.set","params":{"cell":"Summary!H2","value":f"=COUNT(S01!A2:A{r0+n-1})"}})+"\n"+
 json.dumps({"id":"cell.set","params":{"cell":"Summary!H3","value":f"=S01!G{r0+n-1}"}})+"\n")
print("expected appended sumG", round(sg,2), "last row", r0+n-1, "last G", vals[-1][6])
