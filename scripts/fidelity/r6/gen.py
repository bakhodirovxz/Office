"""R6 round-trip inputs (docs/phase5/r6.md): a feature-rich workbook and deck, built with openpyxl / python-pptx.
Synthetic content only. Usage: python -I gen.py <out_dir>. resave.ps1 then lets Excel and PowerPoint re-save them, so the
measured files are Office-authored."""
import sys, os, io
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill, Border, Side, Alignment
from openpyxl.formatting.rule import CellIsRule, ColorScaleRule
from openpyxl.worksheet.datavalidation import DataValidation
from openpyxl.comments import Comment
from openpyxl.chart import BarChart, Reference
from openpyxl.drawing.image import Image as XlImage
from openpyxl.workbook.defined_name import DefinedName
from PIL import Image
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.chart.data import CategoryChartData
from pptx.enum.chart import XL_CHART_TYPE

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
png = io.BytesIO(); Image.new("RGB", (64, 40), (30, 120, 200)).save(png, "PNG"); png.seek(0)
with open(os.path.join(out, "pic.png"), "wb") as f:
    f.write(png.getvalue())

wb = Workbook(); ws = wb.active; ws.title = "Xodimlar"
ws.append(["№", "F.I.Sh.", "Lavozim", "Maosh", "Sana", "Ulush"])
names = ["Aliyev Akmal", "Karimova Nodira", "Тошматов Бахтиёр", "Rahimov Sardor", "Юсупова Дилноза"]
for i, n in enumerate(names, 1):
    ws.append([i, n, "Mutaxassis" if i % 2 else "Bosh mutaxassis", 4_500_000 + i * 250_000, f"2026-0{i}-1{i}", i / 10])
ws.append(["", "Jami", "", "=SUM(D2:D6)", "", "=AVERAGE(F2:F6)"])
for c in ws[1]:
    c.font = Font(bold=True, color="FFFFFF", name="Times New Roman", size=12); c.fill = PatternFill("solid", fgColor="1F4E79")
    c.alignment = Alignment(horizontal="center", vertical="center", wrap_text=True)
thin = Side(style="thin", color="000000")
for row in ws.iter_rows(min_row=1, max_row=7, max_col=6):
    for c in row:
        c.border = Border(left=thin, right=thin, top=thin, bottom=thin)
for r in range(2, 8):
    ws.cell(r, 4).number_format = '#,##0" so\'m"'; ws.cell(r, 6).number_format = "0.0%"
ws.column_dimensions["B"].width = 28; ws.column_dimensions["C"].width = 18; ws.row_dimensions[1].height = 30
ws.merge_cells("A9:F9"); ws["A9"] = "Izoh: sintetik ma'lumot"; ws["A9"].alignment = Alignment(horizontal="center")
ws.freeze_panes = "B2"
ws.conditional_formatting.add("D2:D6", CellIsRule(operator="greaterThan", formula=["5000000"], fill=PatternFill("solid", fgColor="FFC7CE")))
ws.conditional_formatting.add("F2:F6", ColorScaleRule(start_type="min", start_color="FFFFFF", end_type="max", end_color="63BE7B"))
dv = DataValidation(type="list", formula1='"Mutaxassis,Bosh mutaxassis,Rahbar"', allow_blank=True); ws.add_data_validation(dv); dv.add("C2:C6")
ws["B2"].comment = Comment("Sintetik izoh", "R6")
ws["B3"].hyperlink = "https://example.org/r6"; ws["B3"].style = "Hyperlink"
ch = BarChart(); ch.title = "Maosh"; ch.add_data(Reference(ws, min_col=4, min_row=1, max_row=6), titles_from_data=True)
ch.set_categories(Reference(ws, min_col=2, min_row=2, max_row=6)); ws.add_chart(ch, "H2")
ws.add_image(XlImage(os.path.join(out, "pic.png")), "H20")
ws.page_setup.orientation = "landscape"; ws.page_setup.paperSize = ws.PAPERSIZE_A4; ws.print_area = "A1:F9"
ws.oddHeader.center.text = "Xodimlar ro'yxati"; ws.oddFooter.right.text = "&P / &N"
wb.defined_names["Maoshlar"] = DefinedName("Maoshlar", attr_text="Xodimlar!$D$2:$D$6")
s2 = wb.create_sheet("Hisob")
s2["A1"] = "Jami maosh"; s2["B1"] = "=SUM(Xodimlar!D2:D6)"; s2["A2"] = "Nomlangan"; s2["B2"] = "=SUM(Maoshlar)"
s2["A3"] = "Matn"; s2["B3"] = '=CONCATENATE("Jami: ",TEXT(B1,"#,##0"))'
s2.column_dimensions["A"].hidden = True; s2.row_dimensions[5].hidden = True; s2.sheet_properties.tabColor = "C00000"
s3 = wb.create_sheet("Yashirin"); s3["A1"] = 42; s3.sheet_state = "hidden"
s2.protection.sheet = True
wb.save(os.path.join(out, "features.xlsx"))

p = Presentation(); p.slide_width, p.slide_height = Inches(13.333), Inches(7.5)
s = p.slides.add_slide(p.slide_layouts[0]); s.shapes.title.text = "Hisobot 2026"; s.placeholders[1].text = "Sintetik taqdimot"
s.notes_slide.notes_text_frame.text = "Ma'ruzachi izohi"
s = p.slides.add_slide(p.slide_layouts[5]); s.shapes.title.text = "Jadval"
t = s.shapes.add_table(3, 3, Inches(1), Inches(1.5), Inches(8), Inches(2)).table
for r in range(3):
    for c in range(3):
        t.cell(r, c).text = f"R{r}C{c}" if r else ["Ism", "Lavozim", "Maosh"][c]
tb = s.shapes.add_textbox(Inches(1), Inches(4.5), Inches(6), Inches(1)); tb.text_frame.text = "Kirill: Тошкент, Ўзбекистон"
tb.text_frame.paragraphs[0].runs[0].font.bold = True; tb.text_frame.paragraphs[0].runs[0].font.size = Pt(24)
s = p.slides.add_slide(p.slide_layouts[5]); s.shapes.title.text = "Diagramma"
cd = CategoryChartData(); cd.categories = ["I", "II", "III"]; cd.add_series("Reja", (10, 20, 30))
s.shapes.add_chart(XL_CHART_TYPE.COLUMN_CLUSTERED, Inches(1), Inches(1.5), Inches(7), Inches(4.5), cd)
s.shapes.add_picture(os.path.join(out, "pic.png"), Inches(9), Inches(2), Inches(2))
s = p.slides.add_slide(p.slide_layouts[6])
from pptx.enum.shapes import MSO_SHAPE
sh = s.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, Inches(2), Inches(2), Inches(4), Inches(2)); sh.text = "Shakl"
p.save(os.path.join(out, "features.pptx"))
print("ok", out)
