// Excel (docs/ms-addin/PLAN.md section 4.2): cell values, never formulas; rich-text cells run by run; shapes, text
// boxes and notes (legacy comments) through Text2; threaded comments where the build has them. Excel clears its own
// undo list on any programmatic change, so every change is recorded in a Snapshot for "Undo transliteration".
using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace UzTranslit.Office
{
    /// What one command changed, enough to put it back.
    sealed class Snapshot
    {
        public string Doc;
        public readonly List<SpanEdit> Spans = new List<SpanEdit>();     // plain cells, one row span each
        public readonly List<CellEdit> Cells = new List<CellEdit>();     // rich-text cells and notes
        public readonly List<TextEdits> Texts = new List<TextEdits>();
        public readonly List<Action> Other = new List<Action>();   // threaded comments: restore actions
        public int Count { get { int n = Cells.Count + Other.Count; foreach (var s in Spans) n += s.New.Length; foreach (var t in Texts) n += t.Edits.Count; return n; } }
    }

    /// Plain cells written as one row span: sheet, row, first column, the texts before and after.
    sealed class SpanEdit
    {
        public dynamic Sheet;
        public int Row, Col;
        public string[] Old, New;
    }

    sealed class CellEdit
    {
        public dynamic Cell;          // a cell, or a note's TextFrame (both have Characters())
        public Func<string> Read;     // the target's current text
        public string Old, New;
        /// Rich-text cells: the run spans [start (1-based), old text, new text], last run first.
        public List<object[]> Runs;
    }

    static class ExcelAdapter
    {
        /// scope: "selection", "sheet" or "workbook".
        public static Stats Convert(dynamic app, Mode mode, string scope, Snapshot snap)
        {
            if (app.Workbooks.Count == 0) throw new UserMessage("msg.no_document");
            var st = new Stats();
            var clock = Stopwatch.StartNew();
            dynamic wb = app.ActiveWorkbook;
            snap.Doc = wb.FullName;
            bool screen = true;
            object calc = null;
            try { screen = app.ScreenUpdating; app.ScreenUpdating = false; } catch { }
            try { calc = app.Calculation; app.Calculation = -4135; } catch { }   // xlCalculationManual during the writes
            try
            {
                if (scope == "selection")
                {
                    dynamic sel = app.Selection;
                    bool isRange = false;
                    try { isRange = sel.Areas != null; } catch { }
                    if (isRange)
                    {
                        Cells(app, sel, mode, st, snap);
                    }
                    else
                    {
                        // Shapes selected: their text.
                        var ranges = new List<Func<dynamic>>();
                        foreach (dynamic s in sel.ShapeRange) Text2.Shapes(s, ranges);
                        foreach (var r in ranges) Keep(snap, Text2.Convert(r, mode, st));
                    }
                }
                else
                {
                    var sheets = new List<dynamic>();
                    if (scope == "workbook") foreach (dynamic ws in wb.Worksheets) sheets.Add(ws);
                    else sheets.Add(app.ActiveSheet);
                    foreach (dynamic ws in sheets) Sheet(app, ws, mode, st, snap);
                }
            }
            finally
            {
                if (calc != null) try { app.Calculation = calc; } catch { }
                try { app.ScreenUpdating = screen; } catch { }
            }
            st.Ms = clock.ElapsedMilliseconds;
            return st;
        }

        static void Keep(Snapshot snap, TextEdits t) { if (t.Edits.Count > 0) snap.Texts.Add(t); }

        static void Sheet(dynamic app, dynamic ws, Mode mode, Stats st, Snapshot snap)
        {
            if ((bool)ws.ProtectContents) { st.Skipped++; return; }
            st.Stories++;
            Cells(app, ws.UsedRange, mode, st, snap);
            var ranges = new List<Func<dynamic>>();
            foreach (dynamic s in ws.Shapes) Text2.Shapes(s, ranges);
            // Notes (legacy comments): TextFrame2 paragraph/run access is "not implemented" on note shapes (measured),
            // so through the legacy TextFrame.Characters, run by run like a rich-text cell.
            foreach (dynamic c in ws.Comments)
            {
                dynamic cm = c;
                try
                {
                    string t = cm.Text();
                    string neu = Native.Convert(t, mode.To, mode.Alphabet);
                    st.Paragraphs++;
                    if (neu != t) Rich((object)cm.Shape.TextFrame, () => (string)cm.Text(), t, mode, st, snap);
                }
                catch (Exception e) { st.Skipped++; st.Error = e.Message; }
            }
            foreach (var r in ranges)
                try { Keep(snap, Text2.Convert(r, mode, st)); } catch (Exception e) { st.Skipped++; st.Error = e.Message; }
            Threaded(ws, mode, st, snap);
        }

        /// Threaded comments (Microsoft 365 builds only; plain text, no formatting to keep).
        static void Threaded(dynamic ws, Mode mode, Stats st, Snapshot snap)
        {
            dynamic list;
            try { list = ws.CommentsThreaded; } catch { return; }
            foreach (dynamic ct in list)
            {
                var items = new List<dynamic> { ct };
                try { foreach (dynamic rep in ct.Replies) items.Add(rep); } catch { }
                foreach (dynamic it in items)
                {
                    try
                    {
                        string old = it.Text();
                        string neu = Native.Convert(old, mode.To, mode.Alphabet);
                        if (neu == old) continue;
                        it.Text(neu);
                        dynamic keep = it;
                        snap.Other.Add(() => { if ((string)keep.Text() == neu) keep.Text(old); });
                        st.Edits++;
                    }
                    catch { st.Skipped++; }   // e.g. another author's comment
                }
            }
        }

        /// Constant text cells of `range` (intersected with the used range): read as Value2 and Formula arrays.
        static void Cells(dynamic app, dynamic range, Mode mode, Stats st, Snapshot snap)
        {
            dynamic ws = range.Worksheet;
            dynamic used = ws.UsedRange;
            foreach (dynamic area0 in range.Areas)
            {
                dynamic area = app.Intersect(area0, used);
                if (area == null) continue;
                int rows = area.Rows.Count, cols = area.Columns.Count;
                object v = area.Value2, f = area.Formula;
                Func<object, int, int, object> at = (o, r, c) => o is object[,] ? ((object[,])o)[r, c] : o;
                // Uniform font over the area, or over a row: no cell in it can be rich text (Font properties are null
                // when mixed). Per-cell checks only in mixed rows: per-cell COM calls cost ~1 ms each (measured).
                bool uniform = Uniform(area.Font);
                int r0 = area.Row, c0 = area.Column;
                for (int r = 1; r <= rows; r++)
                {
                    bool rowUniform = uniform || Uniform(ws.Range[ws.Cells[r0 + r - 1, c0], ws.Cells[r0 + r - 1, c0 + cols - 1]].Font);
                    // Spans of consecutive plain changed cells in this row: one Value2 write each.
                    int spanStart = 0;
                    var spanVals = new List<string>();
                    var spanOld = new List<string>();
                    for (int c = 1; c <= cols + 1; c++)
                    {
                        string s = null, neu = null;
                        bool plainChange = false;
                        if (c <= cols)
                        {
                            s = at(v, r, c) as string;
                            string fs = at(f, r, c) as string;
                            // Formula() also shows "=..." for text typed with a leading apostrophe: HasFormula decides.
                            if (s != null && fs != null && fs.StartsWith("=") && (bool)ws.Cells[r0 + r - 1, c0 + c - 1].HasFormula) { st.Formulas++; s = null; }
                            if (s != null)
                            {
                                st.Paragraphs++;
                                neu = Native.Convert(s, mode.To, mode.Alphabet);
                                if (neu != s)
                                {
                                    dynamic cell = rowUniform ? null : ws.Cells[r0 + r - 1, c0 + c - 1];
                                    if (!rowUniform && !Uniform(cell.Font)) { dynamic cl = cell; Rich((object)cell, () => cl.Value2 as string, s, mode, st, snap); }
                                    else plainChange = true;
                                }
                            }
                        }
                        if (plainChange)
                        {
                            if (spanVals.Count == 0) spanStart = c;
                            spanVals.Add(neu);
                            spanOld.Add(s);
                            continue;
                        }
                        if (spanVals.Count > 0)
                        {
                            WriteSpan(ws, r0 + r - 1, c0 + spanStart - 1, spanOld, spanVals, st, snap);
                            spanVals = new List<string>();
                            spanOld = new List<string>();
                        }
                    }
                }
            }
        }

        static bool Uniform(dynamic font)
        {
            // A property that differs inside the range comes back as VT_NULL, which dynamic turns into DBNull.
            try
            {
                return Set(font.Name) && Set(font.Size) && Set(font.Bold) && Set(font.Italic) && Set(font.Underline)
                    && Set(font.Color) && Set(font.Strikethrough) && Set(font.Superscript) && Set(font.Subscript);
            }
            catch { return false; }
        }

        static bool Set(object o) { return o != null && !(o is DBNull); 
        }

        /// Writes one row span of new texts, then reads it back: a text Excel parsed as a number, date, formula or
        /// boolean ("May 5" from "Май 5") is written again with the text prefix apostrophe.
        static void WriteSpan(dynamic ws, int row, int col, List<string> old, List<string> vals, Stats st, Snapshot snap)
        {
            int n = vals.Count;
            dynamic span = ws.Range[ws.Cells[row, col], ws.Cells[row, col + n - 1]];
            var arr = new object[1, n];
            for (int i = 0; i < n; i++) arr[0, i] = vals[i];
            span.Value2 = n == 1 ? (object)vals[0] : arr;
            st.Prefixed += FixParsed(ws, row, col, span.Value2, vals);
            snap.Spans.Add(new SpanEdit { Sheet = ws, Row = row, Col = col, Old = old.ToArray(), New = vals.ToArray() });
            st.Edits += n;
        }

        /// `back` = what the span reads after writing `vals`; any cell Excel did not keep as that exact text gets it
        /// again with the text prefix apostrophe. Returns how many.
        static int FixParsed(dynamic ws, int row, int col, object back, IList<string> vals)
        {
            int fixedN = 0;
            for (int i = 0; i < vals.Count; i++)
            {
                object b = back is object[,] ? ((object[,])back)[1, i + 1] : back;
                if (vals[i] == null || (b is string && (string)b == vals[i])) continue;
                ws.Cells[row, col + i].Value2 = "'" + vals[i];
                fixedN++;
            }
            return fixedN;
        }

        /// Undo of one row span: the cells that still hold what the conversion wrote get their old text back, in runs
        /// of consecutive cells (a cell edited since is never rewritten). Returns (restored, skipped).
        static int[] RestoreSpan(SpanEdit e)
        {
            int n = e.New.Length, ok = 0, skip = 0;
            dynamic ws = e.Sheet;
            object now = ws.Range[ws.Cells[e.Row, e.Col], ws.Cells[e.Row, e.Col + n - 1]].Value2;
            int i = 0;
            while (i < n)
            {
                object cur = now is object[,] ? ((object[,])now)[1, i + 1] : now;
                if (!(cur is string) || (string)cur != e.New[i]) { skip++; i++; continue; }
                int j = i;
                while (j < n)
                {
                    object cj = now is object[,] ? ((object[,])now)[1, j + 1] : now;
                    if (!(cj is string) || (string)cj != e.New[j]) break;
                    j++;
                }
                var vals = new string[j - i];
                Array.Copy(e.Old, i, vals, 0, j - i);
                dynamic span = ws.Range[ws.Cells[e.Row, e.Col + i], ws.Cells[e.Row, e.Col + j - 1]];
                if (j - i == 1) span.Value2 = vals[0];
                else { var arr = new object[1, j - i]; for (int k = 0; k < vals.Length; k++) arr[0, k] = vals[k]; span.Value2 = arr; }
                FixParsed(ws, e.Row, e.Col + i, span.Value2, vals);
                ok += j - i;
                i = j;
            }
            return new[] { ok, skip };
        }

        /// A cell whose characters are not all formatted alike: find the runs by bisection on Characters().Font,
        /// convert them as one text, rewrite each changed run (last first) so each keeps its formatting.
        static void Rich(dynamic cell, Func<string> read, string text, Mode mode, Stats st, Snapshot snap)
        {
            var runs = new List<int[]>();   // [start (1-based), length]
            Split(cell, 1, text.Length, runs);
            var texts = new string[runs.Count];
            var conv = new bool[runs.Count];
            for (int i = 0; i < runs.Count; i++) { texts[i] = text.Substring(runs[i][0] - 1, runs[i][1]); conv[i] = true; }
            string[] res = Native.ConvertRuns(texts, conv, mode.To, mode.Alphabet);
            if (res == null) { st.Skipped++; return; }
            var rec = new CellEdit { Cell = cell, Read = read, Old = text, New = string.Concat(res), Runs = new List<object[]>() };
            for (int i = runs.Count - 1; i >= 0; i--)
            {
                if (res[i] == texts[i]) continue;
                RewriteChars(cell, runs[i][0], texts[i], res[i]);
                rec.Runs.Add(new object[] { runs[i][0], texts[i], res[i] });
            }
            if (read() != rec.New) throw new InvalidOperationException("rich text did not take the new text");
            snap.Cells.Add(rec);
            st.Rich++;
            st.Edits++;
        }

        static void Split(dynamic cell, int start, int len, List<int[]> runs)
        {
            if (len <= 0) return;
            if (len == 1 || Uniform(cell.Characters[start, len].Font))
            {
                // Merge with the previous run when the formats match (bisection cuts runs at arbitrary points).
                if (runs.Count > 0 && Sig(cell, runs[runs.Count - 1][0]) == Sig(cell, start)) runs[runs.Count - 1][1] += len;
                else runs.Add(new[] { start, len });
                return;
            }
            int h = len / 2;
            Split(cell, start, h, runs);
            Split(cell, start + h, len - h, runs);
        }

        static string Sig(dynamic cell, int pos)
        {
            dynamic f = cell.Characters[pos, 1].Font;
            return f.Name + "|" + f.Size + "|" + f.Bold + "|" + f.Italic + "|" + f.Underline + "|" + f.Color + "|" + f.Strikethrough + "|" + f.Superscript + "|" + f.Subscript;
        }

        /// Characters(start + head, len).Text = the changed middle of a run (keeps the run's font).
        static void RewriteChars(dynamic cell, int start, string old, string neu)
        {
            int head = 0, tail = 0;
            while (head < old.Length && head < neu.Length && old[head] == neu[head]) head++;
            while (tail < old.Length - head && tail < neu.Length - head && old[old.Length - 1 - tail] == neu[neu.Length - 1 - tail]) tail++;
            if (old.Length - head - tail == 0) { if (tail > 0) tail--; else head--; }
            cell.Characters[start + head, old.Length - head - tail].Text = neu.Substring(head, neu.Length - head - tail);
        }

        /// Undo transliteration: what the conversion wrote, where it is still there. Returns (restored, skipped).
        public static int[] Restore(dynamic app, Snapshot snap)
        {
            int ok = 0, skip = 0;
            bool screen = true;
            try { screen = app.ScreenUpdating; app.ScreenUpdating = false; } catch { }
            try
            {
                for (int i = snap.Cells.Count - 1; i >= 0; i--)
                {
                    var e = snap.Cells[i];
                    try
                    {
                        if (e.Read() != e.New) { skip++; continue; }
                        if (e.Runs == null)
                        {
                            e.Cell.Value2 = e.Old;
                            if ((e.Cell.Value2 as string) != e.Old) e.Cell.Value2 = "'" + e.Old;
                        }
                        else
                        {
                            // Recorded last run first, with pre-conversion starts. Now each run starts later by what
                            // the runs before it grew; restore last first so earlier starts stay valid.
                            for (int k = 0; k < e.Runs.Count; k++)
                            {
                                int delta = 0;
                                for (int m = k + 1; m < e.Runs.Count; m++) delta += ((string)e.Runs[m][2]).Length - ((string)e.Runs[m][1]).Length;
                                RewriteChars(e.Cell, (int)e.Runs[k][0] + delta, (string)e.Runs[k][2], (string)e.Runs[k][1]);
                            }
                        }
                        ok++;
                    }
                    catch { skip++; }
                }
                for (int i = snap.Spans.Count - 1; i >= 0; i--)
                {
                    try { var r = RestoreSpan(snap.Spans[i]); ok += r[0]; skip += r[1]; }
                    catch { skip += snap.Spans[i].New.Length; }
                }
                foreach (var t in snap.Texts) { var r = Text2.Restore(t); ok += r[0]; skip += r[1]; }
                foreach (var a in snap.Other) try { a(); ok++; } catch { skip++; }
            }
            finally { try { app.ScreenUpdating = screen; } catch { } }
            return new[] { ok, skip };
        }
    }
}
