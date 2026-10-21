"""WP14 extra locale cases: built-in ids 14-17 and 22, sub-seconds, weekday and month names, tags (docs/phase3/numlocale.md).
Run: python -I gen_wp14_xlsx.py OUT.xlsx   Column A = label, B = value, C = number format."""
import sys
from openpyxl import Workbook

D = 46307.62783564815  # 2026-10-12 15:04:05
CASES = [
    # openpyxl writes numFmtId 14 for "mm-dd-yy" and 22 for "m/d/yy h:mm"; "m/d/yyyy" is a CUSTOM code (Excel ru shows it month first)
    ("id14", D, "mm-dd-yy"), ("id15", D, "d-mmm-yy"), ("id16", D, "d-mmm"), ("id17", D, "mmm-yy"), ("id22", D, "m/d/yy h:mm"),
    ("custom_mdy", D, "m/d/yyyy"),
    ("mmss_0", 0.0000100, "mm:ss.0"), ("ss_0", 0.0000100, "ss.0"), ("ddd", D, "ddd"), ("dddd", D, "dddd"), ("mmm", D, "mmm"),
    ("mmmm", D, "mmmm"), ("mmmmm", D, "mmmmm"), ("ampm", D, "h:mm AM/PM"),
    ("tag843_date", D, "[$-843]d mmmm yyyy"), ("tag409_date", D, "[$-409]d mmmm yyyy"),
    ("gen_neg", -1234.5, "General"), ("neg_grp", -1234.5, "#,##0.00"), ("fr", 3.75, "# ?/?"), ("sci_neg", -0.000123, "0.00E+00"),
]

def main(out):
    wb = Workbook(); ws = wb.active; ws.title = "L"; ws.column_dimensions["A"].width = 16; ws.column_dimensions["B"].width = 40
    for i, (name, val, fmt) in enumerate(CASES, start=1):
        ws.cell(i, 1, name); c = ws.cell(i, 2, val); c.number_format = fmt; ws.cell(i, 3, fmt)
    wb.save(out)

if __name__ == "__main__":
    main(sys.argv[1])
