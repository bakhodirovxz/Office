// Office shape text (TextRange2 from Shape.TextFrame2, Office 2007+): Excel shapes and all of PowerPoint
// (docs/ms-addin/PLAN.md sections 4.2, 4.3). Each paragraph's runs are converted as one text (uzt_convert_runs), and
// only the changed middle of each changed run is rewritten through Paragraph.Characters(start, len).Text, which keeps
// that run's formatting. Edits are addressed by character position inside the paragraph, not by run index: Office
// may split runs differently after an edit (measured in Excel), so a run index is not stable for Undo.
using System;
using System.Collections.Generic;

namespace UzTranslit.Office
{
    /// One rewritten run: paragraph p (1-based), the run's start in the paragraph before the conversion (1-based),
    /// its text before and after.
    sealed class RunEdit
    {
        public int Para, Start;
        public string Old, New;
    }

    /// The edits of one text frame; Range re-reads the TextRange2 from the shape (it may be gone at undo time).
    sealed class TextEdits
    {
        public Func<dynamic> Range;
        public readonly List<RunEdit> Edits = new List<RunEdit>();   // per paragraph, last run first
        public readonly Dictionary<int, string> ParaNew = new Dictionary<int, string>();
    }

    static class Text2
    {
        /// Converts every paragraph of the text range; returns the edits (empty if nothing changed).
        public static TextEdits Convert(Func<dynamic> range, Mode mode, Stats st)
        {
            var rec = new TextEdits { Range = range };
            dynamic tr = range();
            int pc = tr.Paragraphs.Count;
            for (int p = pc; p >= 1; p--)
            {
                dynamic para = tr.Paragraphs[p];
                int rc = para.Runs.Count;
                if (rc == 0) continue;
                var texts = new string[rc];
                var conv = new bool[rc];
                var starts = new int[rc];
                int at = 1;
                for (int r = 1; r <= rc; r++)
                {
                    texts[r - 1] = (string)para.Runs[r].Text ?? "";
                    conv[r - 1] = true;
                    starts[r - 1] = at;
                    at += texts[r - 1].Length;
                }
                string[] res = Native.ConvertRuns(texts, conv, mode.To, mode.Alphabet);
                st.Paragraphs++;
                if (res == null) { st.Skipped++; continue; }
                bool any = false;
                for (int r = rc; r >= 1; r--)
                {
                    if (res[r - 1] == texts[r - 1]) continue;
                    RewriteAt(para, starts[r - 1], texts[r - 1], res[r - 1]);
                    rec.Edits.Add(new RunEdit { Para = p, Start = starts[r - 1], Old = texts[r - 1], New = res[r - 1] });
                    st.Edits++;
                    any = true;
                }
                if (any) rec.ParaNew[p] = (string)tr.Paragraphs[p].Text;
            }
            return rec;
        }

        /// In `range` (anything with Characters(start, len)), the span starting at `start` held `old`; write `neu`,
        /// replacing only the part that differs (paragraph marks and spaces at the ends stay untouched). Never an empty
        /// span: Characters(start, 0) does not reliably insert.
        public static void RewriteAt(dynamic range, int start, string old, string neu)
        {
            int head = 0, tail = 0;
            while (head < old.Length && head < neu.Length && old[head] == neu[head]) head++;
            while (tail < old.Length - head && tail < neu.Length - head && old[old.Length - 1 - tail] == neu[neu.Length - 1 - tail]) tail++;
            if (old.Length - head - tail == 0)
            {
                if (tail > 0) tail--; else head--;
            }
            range.Characters[start + head, old.Length - head - tail].Text = neu.Substring(head, neu.Length - head - tail);
        }

        /// Reverses the edits of every paragraph that still reads exactly as the conversion left it; returns
        /// (restored edits, skipped edits).
        public static int[] Restore(TextEdits rec)
        {
            int ok = 0, skip = 0;
            dynamic tr;
            try { tr = rec.Range(); } catch { return new[] { 0, rec.Edits.Count }; }
            foreach (var p in rec.ParaNew.Keys)
            {
                var edits = rec.Edits.FindAll(e => e.Para == p);   // last run first
                try
                {
                    dynamic para = tr.Paragraphs[p];
                    if ((string)para.Text != rec.ParaNew[p]) { skip += edits.Count; continue; }
                    for (int k = 0; k < edits.Count; k++)
                    {
                        // Now each run starts later by what the runs before it grew (they come later in the list).
                        int delta = 0;
                        for (int m = k + 1; m < edits.Count; m++) delta += edits[m].New.Length - edits[m].Old.Length;
                        RewriteAt(para, edits[k].Start + delta, edits[k].New, edits[k].Old);
                        ok++;
                    }
                }
                catch { skip += edits.Count; }
            }
            return new[] { ok, skip };
        }

        /// Shape text, recursing into groups, tables and SmartArt; `add` gets each text range accessor.
        public static void Shapes(dynamic shape, List<Func<dynamic>> add)
        {
            int type = 0;
            try { type = shape.Type; } catch { }
            if (type == 4) return;   // msoComment: Excel notes are handled with the cells (ExcelAdapter)
            if (type == 6)            // msoGroup
            {
                foreach (dynamic s in shape.GroupItems) Shapes(s, add);
                return;
            }
            try
            {
                if ((int)shape.HasTable != 0)
                {
                    dynamic t = shape.Table;
                    int rows = t.Rows.Count, cols = t.Columns.Count;
                    for (int r = 1; r <= rows; r++)
                        for (int c = 1; c <= cols; c++)
                        {
                            dynamic cell = t.Cell(r, c);
                            add.Add(() => cell.Shape.TextFrame2.TextRange);
                        }
                    return;
                }
            }
            catch { }   // Excel shapes have no HasTable
            try
            {
                if ((int)shape.HasSmartArt != 0)
                {
                    dynamic nodes = shape.SmartArt.AllNodes;
                    int n = nodes.Count;
                    for (int i = 1; i <= n; i++) { dynamic node = nodes.Item(i); add.Add(() => node.TextFrame2.TextRange); }
                    return;
                }
            }
            catch { }
            try
            {
                bool frame = true;
                try { frame = (int)shape.HasTextFrame != 0; } catch { }   // Excel shapes have no HasTextFrame
                if (frame && (int)shape.TextFrame2.HasText != 0)
                    add.Add(() => shape.TextFrame2.TextRange);
            }
            catch { }
        }
    }
}
