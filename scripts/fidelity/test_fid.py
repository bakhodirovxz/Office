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

    def test_derived_losses_are_explained_by_any_loss_warning(self):
        text = [{"feature": "text", "before": 1.0, "after": 0.7}, {"feature": "pagination", "before": 2, "after": 3}]
        self.assertEqual(S.silent_losses(text, []), text)                                        # nothing said at all: silent
        self.assertEqual(S.silent_losses(text, [{"code": "pdf_en_us", "text": "Numbers use en-US"}]), [])
        self.assertEqual(S.silent_losses(text, [{"code": "upstream", "text": "Warning: charts not imported"}]), [])
        # a warning that announces no loss explains nothing
        self.assertEqual(S.silent_losses(text, [{"code": "text_transcoded", "text": "cp1251"}, {"code": "kind_mismatch", "text": "x"}]), text)
        # a structural loss needs its own warning: "text" in a warning does not cover a lost comment
        c = [{"feature": "comments", "before": 1, "after": 0}]
        self.assertEqual(S.silent_losses(c, [{"code": "pdf_en_us", "text": "Numbers use en-US"}]), c)
        self.assertEqual(S.silent_losses(c, [{"code": "comments_dropped", "text": "Comments are not kept in the result."}]), [])

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
