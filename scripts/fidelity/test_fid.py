"""Self-checks for the harness logic that has no Office dependency:  python -I scripts/fidelity/test_fid.py"""
import sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fid import structure as S                      # noqa: E402
from fid.p2d import width_error                     # noqa: E402
from fid.util import text_metrics                   # noqa: E402


class T(unittest.TestCase):
    def test_silent_losses(self):
        losses = [{"feature": "comments", "before": 1, "after": 0}, {"feature": "charts", "before": 1, "after": 0}]
        warn = [{"code": "upstream", "text": "Warning: comments are not imported"}]
        self.assertEqual([l["feature"] for l in S.silent_losses(losses, warn)], ["charts"])
        self.assertEqual(len(S.silent_losses(losses, [])), 2)

    def test_derived_losses_are_explained_only_by_a_warning_that_names_them(self):
        text = [{"feature": "text", "before": 1.0, "after": 0.7}, {"feature": "pagination", "before": 2, "after": 3}]
        self.assertEqual(S.silent_losses(text, []), text)                                        # nothing said at all: silent
        self.assertEqual(S.silent_losses(text, [{"code": "pdf_en_us", "text": "Numbers use en-US"}]), text[1:])   # en-US explains text, not pages
        self.assertEqual(S.silent_losses(text, [{"code": "headers_footers_dropped", "text": "Headers are not kept"}]), [])
        self.assertEqual(S.silent_losses(text, [{"code": "upstream", "text": "Warning: charts not imported"}]), [])
        # a warning that names another kind of loss, or none, explains nothing: a second, unwarned text loss stays silent
        self.assertEqual(S.silent_losses(text, [{"code": "macros_dropped", "text": "Macros are not kept"}]), text)
        self.assertEqual(S.silent_losses(text, [{"code": "upstream", "text": "Warning: the font was substituted"}]), text)
        self.assertEqual(S.silent_losses(text, [{"code": "text_transcoded", "text": "cp1251"}, {"code": "kind_mismatch", "text": "x"}]), text)
        cyr = [{"feature": "cyrillic", "before": 100, "after": 90}]
        self.assertEqual(S.silent_losses(cyr, [{"code": "pdf_en_us", "text": "x"}]), cyr)
        self.assertEqual(S.silent_losses(cyr, [{"code": "pdf_text_missing", "text": "18 of 480 letters"}]), [])
        # a structural loss needs its own warning: "text" in a warning does not cover a lost comment
        c = [{"feature": "comments", "before": 1, "after": 0}]
        self.assertEqual(S.silent_losses(c, [{"code": "pdf_en_us", "text": "Numbers use en-US"}]), c)
        self.assertEqual(S.silent_losses(c, [{"code": "comments_dropped", "text": "Comments are not kept in the result."}]), [])

    def test_a_lost_pdf_column_is_a_text_loss_unless_the_report_says_so(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("fid_run", Path(__file__).resolve().parent / "run.py")
        run = importlib.util.module_from_spec(spec); spec.loader.exec_module(run)
        pdf = " ".join(["Rahimov Sherzod Toshpulatovich"] * 12 + ["Karimova Dilnoza"] * 12)
        docx = " ".join(["Rahimov Sherzod Toshpulatovich"] * 12)
        L = run.losses_of(None, None, None, text_metrics(pdf, docx))
        self.assertIn("text", [l["feature"] for l in L])
        self.assertEqual([l["feature"] for l in S.silent_losses(L, [])], [l["feature"] for l in L])
        self.assertEqual(S.silent_losses(L, [{"code": "pdf_text_missing", "text": "18 of 480 letters are missing"}]), [])
        self.assertEqual(run.losses_of(None, None, None, text_metrics(pdf, pdf)), [])

    def test_csv_compare_reads_each_file_with_its_own_delimiter(self):
        import tempfile
        spec = __import__("importlib.util").util.spec_from_file_location("fid_run2", Path(__file__).resolve().parent / "run.py")
        run = __import__("importlib.util").util.module_from_spec(spec); spec.loader.exec_module(run)
        with tempfile.TemporaryDirectory() as d:
            ru = Path(d) / "ru.csv"; en = Path(d) / "en.csv"; ru2 = Path(d) / "ru2.csv"
            ru.write_text("Name;Pay;Date\nA;3500000,0;01.03.2019\nB;12,5;11.09.2019\n", "utf-8")
            en.write_text('Name,Pay,Date\nA,3500000.0,01/03/2019\nB,"12.5",11/09/2019\n', "utf-8")
            ru2.write_text("Name;Pay;Date;;\nA;3500000,0;01.03.2019;;\nB;12,5;11.09.2019;;\n", "utf-8-sig")
            same = run.csv_compare(ru, ru2)            # Excel pads rows; the same content in the same locale is equal
            self.assertEqual((same["cells_equal"], same["cells_other_diff"], same["delimiter_ref"]), (9, 0, ";"))
            other = run.csv_compare(ru, en)            # ru against en-US: only locale marks differ, and it is counted apart
            self.assertEqual((other["delimiter_ref"], other["delimiter_cand"], other["cells_other_diff"]), (";", ",", 0))
            self.assertLess(other["cell_equality"], 1.0)
            self.assertEqual(other["cell_equality_locale_neutral"], 1.0)

    def test_one_office_hang_costs_one_file(self):
        """A stand-in office_ref.ps1 hangs on 'hang.docx'. The batch kills only the process in the pid file, marks that file as a timeout and
        runs the files after it in a fresh instance."""
        import subprocess, tempfile
        from fid import refs
        if not (hasattr(subprocess, "STARTUPINFO") and subprocess.run(["where", "powershell"], capture_output=True).returncode == 0):
            self.skipTest("needs Windows PowerShell")
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d / "office_ref.ps1").write_text(r'''
param([string]$App, [string]$List, [string]$Result, [string]$Progress = "")
$items = [IO.File]::ReadAllText($List, [Text.Encoding]::UTF8) | ConvertFrom-Json
if ($items -isnot [array]) { $items = @($items) }
$child = Start-Process -PassThru -WindowStyle Hidden ping -ArgumentList "-n 200 127.0.0.1"
Set-Content $env:FID_PIDFILE $child.Id
$all = @()
foreach ($it in $items) {
    if ($it.src -like "*hang.docx") { Start-Sleep 300 }
    $r = [ordered]@{ src = $it.src; out = $it.out; fmt = $it.fmt; ok = $true; error = $null; ms = 1 }
    $all += $r
    [IO.File]::AppendAllText($Progress, (ConvertTo-Json -InputObject $r -Compress) + "`n")
}
Stop-Process -Id $child.Id -Force
[IO.File]::WriteAllText($Result, (ConvertTo-Json -InputObject @($all)))
''', "ascii")
            old = refs.HERE
            refs.HERE = d
            try:
                jobs = [{"src": "C:/x/a.docx", "out": "C:/x/a.pdf", "fmt": "pdf"}, {"src": "C:/x/hang.docx", "out": "C:/x/h.pdf", "fmt": "pdf"},
                        {"src": "C:/x/b.docx", "out": "C:/x/b.pdf", "fmt": "pdf"}]
                res = refs.office_batch("word", jobs, d / "work", "t", per_item=4)
            finally:
                refs.HERE = old
        self.assertTrue(res["C:/x/a.pdf"]["ok"])
        self.assertFalse(res["C:/x/h.pdf"]["ok"])
        self.assertTrue(res["C:/x/h.pdf"]["error"].endswith("timeout"))
        self.assertTrue(res["C:/x/b.pdf"]["ok"], res["C:/x/b.pdf"])

    def test_rtf_structure_counts(self):
        import tempfile
        rtf = ("{\\rtf1\\ansi{\\header\\pard HRM\\par}{\\headerf\\pard\\plain\\par}"
               "\\pard\\intbl a\\cell\\row\\trowd\\pard\\intbl b\\cell\\row\\pard text\\par"
               "\\trowd\\clvmgf\\pard\\intbl c\\cell\\row"
               "{\\*\\atrfstart x}{\\*\\annotation c}{\\shppict{\\pict a}}{\\nonshppict{\\pict a}}}")
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "a.rtf"; f.write_bytes(rtf.encode("latin-1"))
            r = S.structure(f)
        # two tables (an ordinary paragraph ends the first), one comment, one picture, one header with text (not the empty first-page one)
        self.assertEqual((r["tables"], r["comments"], r["images"], r["headers_footers"], r["merged"]), (2, 1, 1, 1, 1))

    def test_docx_merge_counts_a_vertical_merge_once(self):
        import tempfile, zipfile
        xml = ('<w:document><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr></w:tc></w:tr>'
               '<w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr></w:tc></w:tr></w:tbl></w:document>')
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "a.docx"
            with zipfile.ZipFile(f, "w") as z: z.writestr("word/document.xml", xml)
            self.assertEqual(S.structure(f)["merged"], 2)

    def test_text_loss_needs_missing_words_and_translit_pages_have_a_tolerance(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("fid_run", Path(__file__).resolve().parent / "run.py")
        run = importlib.util.module_from_spec(spec); spec.loader.exec_module(run)
        order_only = {"word_sim": 0.6, "missing_count": 0, "ref_words": 300, "chars_kept": 1.0}                       # same words, another reading order
        self.assertEqual(run.losses_of(None, None, None, order_only), [])
        lost = {"word_sim": 0.6, "missing_count": 40, "ref_words": 300, "chars_kept": 0.8}
        glued = {"word_sim": 0.88, "missing_count": 25, "ref_words": 300, "chars_kept": 1.0}               # words glued by a tab in the extraction
        self.assertEqual(run.losses_of(None, None, None, glued), [])
        self.assertEqual([l["feature"] for l in run.losses_of(None, None, None, lost)], ["text"])
        pages = {"page_match": False, "pages_ref": 108, "pages_cand": 113}
        self.assertEqual(run.losses_of(None, None, pages, None, translit=True), [])                 # 4.6% longer: expected
        self.assertEqual([l["feature"] for l in run.losses_of(None, None, pages, None)], ["pagination"])
        self.assertEqual([l["feature"] for l in run.losses_of(None, None, {**pages, "pages_cand": 140}, None, translit=True)], ["pagination"])

    def test_structural_losses(self):
        a, b = {"tables": 2, "images": 1, "sheets": 3}, {"tables": 2, "images": 0}
        self.assertEqual([l["feature"] for l in S.structural_losses(a, b)], ["images", "sheets"])
        self.assertEqual([l["feature"] for l in S.structural_losses(a, b, "csv")], ["sheets"])

    def test_width_error(self):
        self.assertEqual(width_error([[30, 190, 130, 90]], [[60, 380, 260, 180]]), 0.0)     # same fractions
        self.assertIsNone(width_error([[30, 190, 130, 90]], [[100, 100]]))                  # column count differs
        self.assertGreater(width_error([[30, 190, 130, 90]], [[100, 100, 100, 100]]), 0.3)

    def test_text_metrics_cyrillic(self):
        m = text_metrics("Буйруқ тест", "Буйруқ ?????")
        self.assertLess(m["cyr_kept"], 1.0)
        self.assertEqual(text_metrics("a b c", "a b c")["word_sim"], 1.0)
        self.assertEqual(text_metrics("Lavozim	Summa 1", "Lavozim Summa 1")["chars_kept"], 1.0)    # a glued word is not lost text
        self.assertLess(text_metrics("one two three four", "one two")["chars_kept"], 0.6)


if __name__ == "__main__":
    unittest.main()
