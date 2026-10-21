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


if __name__ == "__main__":
    unittest.main()
