// Word: converts the selection, or every story of the document, in place and keeps the formatting
// (docs/ms-addin/PLAN.md section 4.1). The whole command is one UndoRecord (Word 2010+); on failure it is undone.
//
// Main text, plain paragraphs (WordXml): groups of whole paragraphs are read with Range.WordOpenXML, every paragraph
// that passes the allow-list is converted in the XML and written back with Range.InsertXML; the text read back must
// equal the expected text exactly, else the command fails and is undone.
// Everything else, per run: walk the formatting runs (Range.MoveEnd(wdCharacterFormatting)), cut them at field code
// spans, bookmark/comment/content-control edges and control characters; each piece is text of one formatting, so
// writing its new text through Range.Text keeps that formatting. Pieces are grouped into paragraphs and converted with
// uz_translit convert_runs (its output does not depend on where a paragraph is cut). Writes go from the end of the
// story to the start, so earlier positions stay valid.
using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace UzTranslit.Office
{
    sealed class Stats
    {
        public int Paragraphs, Pieces, Edits, Skipped, Stories, XmlParagraphs, XmlInserts, XmlRefused, XmlMismatch, Formulas, Rich, Prefixed;
        public string Refused = "", Error = "";
        public long Ms;
        public override string ToString()
        {
            return "paragraphs=" + Paragraphs + ";pieces=" + Pieces + ";edits=" + Edits + ";skipped=" + Skipped
                + ";stories=" + Stories + ";xml_paragraphs=" + XmlParagraphs + ";xml_inserts=" + XmlInserts + ";xml_refused=" + XmlRefused + ";xml_mismatch=" + XmlMismatch + (Refused.Length > 0 ? "(" + Refused + ")" : "") + ";formulas=" + Formulas + ";rich=" + Rich + ";prefixed=" + Prefixed + ";ms=" + Ms + (Error.Length > 0 ? ";last_error=" + Error : "");
        }
    }

    static class WordAdapter
    {
        const int wdCharacterFormatting = 13, wdNoProtection = -1, wdMainTextStory = 1;
        /// Paragraphs per WordOpenXML read on the fast path.
        const int XmlGroup = 50;

        struct Piece
        {
            public int Start, End;
            public string Text;
            public bool Convert, ParagraphEnd;
        }

        struct Edit
        {
            public int Start, End;
            public string Text;
        }

        /// fast: plain paragraphs of the main text through WordXml (about 50x faster on long documents) - but
        /// Range.WordOpenXML and Range.InsertXML split the custom undo record (measured, Word 2019), so the command is
        /// then several Undo steps. Off by default; decision D8 (docs/ms-addin/PLAN.md).
        public static Stats Convert(dynamic app, Mode mode, bool wholeDocument, string undoLabel, bool fast)
        {
            if (app.Documents.Count == 0) throw new UserMessage("msg.no_document");
            dynamic doc = app.ActiveDocument;
            if (doc.ProtectionType != wdNoProtection) throw new UserMessage("msg.protected");
            dynamic sel = app.Selection;
            bool whole = wholeDocument || sel.Start == sel.End;
            if (whole) mode = mode.Whole();
            if ((whole ? doc.Revisions.Count : sel.Range.Revisions.Count) > 0) throw new UserMessage("msg.tracked");
            // With Track Changes on, InsertXML would record whole paragraphs as replaced: per run only.
            bool xml = fast && !(bool)doc.TrackRevisions;

            var st = new Stats();
            var clock = Stopwatch.StartNew();
            var stories = new List<dynamic>();
            if (whole)
            {
                foreach (dynamic s in doc.StoryRanges)
                    for (dynamic r = s; r != null; r = r.NextStoryRange) stories.Add(r);
                // Text boxes in headers and footers are not on the NextStoryRange chain. Converting a story twice is
                // harmless: the conversion is idempotent.
                try
                {
                    foreach (dynamic shp in doc.Sections[1].Headers[1].Shapes)
                        try { if (shp.TextFrame.HasText != 0) stories.Add(shp.TextFrame.TextRange); } catch { }
                }
                catch { }
            }
            else stories.Add(sel.Range);

            bool record = false, screen = true;
            dynamic undo = null;
            try { undo = app.UndoRecord; undo.StartCustomRecord(undoLabel); record = true; } catch { }
            try { screen = app.ScreenUpdating; app.ScreenUpdating = false; } catch { }
            // Per-edit cost: background repagination and as-you-type proofing re-run after every write. Off for the
            // command, restored in finally (they are user options; a crash mid-command would leave them). Draft view
            // was tried too: it recomputed PAGE fields in footers (smoke test), so the view is left alone.
            dynamic opts = app.Options;
            bool pag = true, spell = true, gram = true, smart = true;
            try { pag = opts.Pagination; spell = opts.CheckSpellingAsYouType; gram = opts.CheckGrammarAsYouType; smart = opts.SmartCutPaste; } catch { }
            try { opts.Pagination = false; opts.CheckSpellingAsYouType = false; opts.CheckGrammarAsYouType = false; } catch { }
            // Smart cut and paste adjusts spaces at the edges of copied/pasted text; WordOpenXML and InsertXML go
            // through it (a range's trailing space was missing from the export in the benchmark).
            if (xml) try { opts.SmartCutPaste = false; } catch { }
            dynamic bookmarks = doc.Bookmarks;
            bool hidden = bookmarks.ShowHidden;
            bookmarks.ShowHidden = true;
            try
            {
                foreach (dynamic s in stories)
                {
                    st.Stories++;
                    int from = s.Start, to = s.End;
                    if (xml && (int)s.StoryType == wdMainTextStory) Main(s, from, to, (int)doc.Content.End, mode, st);
                    else PerRun(s, from, to, mode, st);
                }
            }
            catch
            {
                bool one = record;
                if (record) { try { undo.EndCustomRecord(); } catch { } record = false; }
                if (st.Edits + st.XmlInserts > 0) try { doc.Undo(one ? 1 : st.Edits + st.XmlInserts); } catch { }
                throw;
            }
            finally
            {
                try { bookmarks.ShowHidden = hidden; } catch { }
                if (record) try { undo.EndCustomRecord(); } catch { }
                try { opts.Pagination = pag; opts.CheckSpellingAsYouType = spell; opts.CheckGrammarAsYouType = gram; } catch { }
                if (xml) try { opts.SmartCutPaste = smart; } catch { }
                try { app.ScreenUpdating = screen; } catch { }
            }
            st.Ms = clock.ElapsedMilliseconds;
            return st;
        }

        /// The main text: whole paragraphs inside [from, to) except the story's last one go through WordXml in groups;
        /// the rest (partial paragraphs at the selection edges, the last paragraph, non-plain paragraphs) per run.
        static void Main(dynamic story, int from, int to, int storyEnd, Mode mode, Stats st)
        {
            dynamic rng = story.Duplicate;
            rng.SetRange(from, to);
            var ps = new List<int[]>();
            // [start, end, in a table]: a cell's paragraph mark is the cell mark, never written through InsertXML.
            foreach (dynamic p in rng.Paragraphs) { dynamic pr = p.Range; ps.Add(new[] { (int)pr.Start, (int)pr.End, (bool)pr.Information[12] ? 1 : 0 }); }
            // Segments, last to first: [i, j) of whole candidate paragraphs (chunked), or a per-run span.
            int k = ps.Count;
            while (k > 0)
            {
                int j = k;
                bool cand = Candidate(ps[j - 1], from, to, storyEnd);
                while (k > 0 && Candidate(ps[k - 1], from, to, storyEnd) == cand && (!cand || j - k < XmlGroup)) k--;
                if (cand) XmlParagraphs(story, ps, k, j, mode, st);
                else PerRun(story, Math.Max(ps[k][0], from), Math.Min(ps[j - 1][1], to), mode, st);
            }
        }

        static bool Candidate(int[] p, int from, int to, int storyEnd)
        {
            return p[0] >= from && p[1] <= to && p[1] < storyEnd && p[2] == 0;
        }

        /// Paragraphs ps[i..j) through WordXml; a package that is not paragraph-for-paragraph (a table, a block
        /// content control) is bisected, a single such paragraph goes per run.
        static void XmlParagraphs(dynamic story, List<int[]> ps, int i, int j, Mode mode, Stats st)
        {
            dynamic r = story.Duplicate;
            // Every XML range ends just before the last paragraph mark (WordXml.Convert says why).
            r.SetRange(ps[i][0], ps[j - 1][1] - 1);
            var x = WordXml.Load((string)r.WordOpenXML);
            bool[] plain = WordXml.Classify(x, j - i);
            if (plain == null)
            {
                if (j - i == 1) { PerRun(story, ps[i][0], ps[i][1], mode, st); return; }
                int m = (i + j) / 2;
                XmlParagraphs(story, ps, m, j, mode, st);
                XmlParagraphs(story, ps, i, m, mode, st);
                return;
            }
            int k = j;
            while (k > i)
            {
                int e = k;
                bool c = plain[k - 1 - i];
                while (k > i && plain[k - 1 - i] == c) k--;
                if (!c) { PerRun(story, ps[k][0], ps[e - 1][1], mode, st); continue; }
                int s = ps[k][0], end = ps[e - 1][1] - 1;
                r.SetRange(s, end);
                string old = r.Text;
                // The exact span: a package cut out of a larger read was refused by InsertXML (0x800A1801) in tests.
                var sub = k == i && e == j ? x : WordXml.Load((string)r.WordOpenXML);
                string want, source;
                string pkg = WordXml.Convert(sub, e - k, mode, out want, out source);
                // The package must hold exactly the text Word shows, or what it lost would be lost for good.
                if (pkg == null || source != old) { st.XmlMismatch++; PerRun(story, ps[k][0], ps[e - 1][1], mode, st); continue; }
                if (old == want) { st.Paragraphs += e - k; continue; }   // nothing changes
                try { r.InsertXML(pkg); }
                catch (System.Runtime.InteropServices.COMException ex)
                {
                    // Word refused this location: if nothing changed, convert it per run instead.
                    r.SetRange(s, end);
                    if ((string)r.Text != old) throw;
                    st.XmlRefused++;
                    st.Refused = "0x" + ex.ErrorCode.ToString("X8") + "@" + ps[k][0];
                    PerRun(story, ps[k][0], ps[e - 1][1], mode, st);
                    continue;
                }
                st.XmlInserts++;
                st.XmlParagraphs += e - k;
                st.Paragraphs += e - k;
                // The new text, then the untouched last paragraph mark. (Ending the range before trailing spaces
                // instead made InsertXML add a paragraph mark: the range must end exactly at the mark.)
                r.SetRange(s, s + want.Length + 1);
                string got = r.Text, exp = want + "\r";
                if (got != exp)
                {
                    int d = 0;
                    while (d < got.Length && d < exp.Length && got[d] == exp[d]) d++;
                    int a = Math.Max(0, d - 20);
                    throw new InvalidOperationException("InsertXML changed the text at " + (s + d) + ": '"
                        + Show(got.Substring(a, Math.Min(40, got.Length - a))) + "' instead of '" + Show(exp.Substring(a, Math.Min(40, exp.Length - a))) + "'");
                }
            }
        }

        static string Show(string s)
        {
            var b = new System.Text.StringBuilder();
            foreach (char c in s) if (c < ' ') b.Append("<" + (int)c + ">"); else b.Append(c);
            return b.ToString();
        }

        /// Positions the pieces must be cut at, and the spans that are never edited (field codes and field marks).
        static void Boundaries(dynamic range, SortedSet<int> cuts, List<int[]> locked)
        {
            foreach (dynamic f in range.Fields)
            {
                int cs = f.Code.Start, ce = f.Code.End;
                dynamic res = f.Result;
                int rs = res.Start, re = res.End;
                if (rs >= ce) { locked.Add(new[] { cs - 1, rs }); locked.Add(new[] { re, re + 1 }); }
                else locked.Add(new[] { cs - 1, ce + 1 });
            }
            foreach (dynamic b in range.Bookmarks) { cuts.Add((int)b.Start); cuts.Add((int)b.End); }
            try { foreach (dynamic c in range.Comments) { dynamic sc = c.Scope; cuts.Add((int)sc.Start); cuts.Add((int)sc.End); } } catch { }
            try
            {
                foreach (dynamic cc in range.ContentControls)
                {
                    dynamic r = cc.Range;
                    int a = r.Start, b = r.End;
                    locked.Add(new[] { a - 1, a });
                    locked.Add(new[] { b, b + 1 });
                }
            }
            catch { }
            // Equations: variable names and operators are not Uzbek text.
            try { foreach (dynamic m in range.OMaths) { dynamic r = m.Range; locked.Add(new[] { (int)r.Start, (int)r.End }); } } catch { }
            foreach (var l in locked) { cuts.Add(l[0]); cuts.Add(l[1]); }
        }

        static bool IsLocked(List<int[]> locked, int a, int b)
        {
            foreach (var l in locked) if (a >= l[0] && b <= l[1]) return true;
            return false;
        }

        static void PerRun(dynamic story, int from, int to, Mode mode, Stats st)
        {
            if (to <= from) return;
            dynamic r = story.Duplicate;
            r.SetRange(from, to);
            var cuts = new SortedSet<int>();
            var locked = new List<int[]>();
            Boundaries(r, cuts, locked);

            var pieces = new List<Piece>();
            int pos = from;
            while (pos < to)
            {
                r.SetRange(pos, pos);
                r.MoveEnd(wdCharacterFormatting, 1);
                int end = Math.Min(Math.Max((int)r.End, pos + 1), to);
                // Cut [pos, end) at the boundaries inside it.
                int a = pos;
                if (end - pos > 1)
                    foreach (int c in cuts.GetViewBetween(pos + 1, end - 1)) { Read(r, a, c, locked, pieces, st); a = c; }
                Read(r, a, end, locked, pieces, st);
                pos = end;
            }

            // Group into paragraphs, convert, collect edits.
            var edits = new List<Edit>();
            int p0 = 0;
            for (int i = 0; i <= pieces.Count; i++)
            {
                if (i < pieces.Count && !pieces[i].ParagraphEnd) continue;
                int p1 = i < pieces.Count ? i + 1 : i;
                ConvertParagraph(pieces, p0, p1, mode, edits, st);
                p0 = p1;
            }
            st.Pieces += pieces.Count;

            dynamic w = story.Duplicate;
            for (int i = edits.Count - 1; i >= 0; i--)
            {
                Edit e = edits[i];
                if (e.End - e.Start >= 2 && e.Text.Length > 0 && (cuts.Contains(e.Start) || cuts.Contains(e.End)))
                {
                    // A bookmark, comment or content-control edge is at the edit: replacing the whole span would collapse
                    // it. Insert the new text after the first old character (it takes that character's formatting),
                    // then delete the old characters on both sides; the edges never see their content vanish.
                    int n = e.Text.Length;
                    w.SetRange(e.Start + 1, e.Start + 1);
                    w.Text = e.Text;
                    w.SetRange(e.Start + 1 + n, e.End + n);
                    w.Text = "";
                    w.SetRange(e.Start, e.Start + 1);
                    w.Text = "";
                }
                else
                {
                    w.SetRange(e.Start, e.End);
                    w.Text = e.Text;
                }
                st.Edits++;
            }
        }

        /// One formatting-uniform span [a, b) -> pieces. Locked or unreadable spans become verbatim pieces.
        static void Read(dynamic r, int a, int b, List<int[]> locked, List<Piece> pieces, Stats st)
        {
            if (b <= a) return;
            if (IsLocked(locked, a, b)) { pieces.Add(new Piece { Start = a, End = b, Text = "" }); return; }
            r.SetRange(a, b);
            string t = r.Text ?? "";
            // An end-of-cell or end-of-row mark is one position that reads as "\r\a".
            t = t.Replace("\r\a", "\a");
            if (t.Length != b - a)
            {
                // Objects, field marks, hidden structure: never edited.
                st.Skipped++;
                pieces.Add(new Piece { Start = a, End = b, Text = "", ParagraphEnd = t.IndexOf('\r') >= 0 || t.IndexOf('\a') >= 0 });
                return;
            }
            int s = 0;
            for (int i = 0; i <= t.Length; i++)
            {
                bool ctl = i < t.Length && t[i] < ' ' && t[i] != '\t';
                if (i < t.Length && !ctl) continue;
                if (i > s) pieces.Add(new Piece { Start = a + s, End = a + i, Text = t.Substring(s, i - s), Convert = true });
                if (ctl)
                {
                    char c = t[i];
                    pieces.Add(new Piece { Start = a + i, End = a + i + 1, Text = c.ToString(),
                        ParagraphEnd = c == '\r' || c == '\a' || c == '\f' || c == '\x0e' });
                }
                s = i + 1;
            }
        }

        static void ConvertParagraph(List<Piece> pieces, int p0, int p1, Mode mode, List<Edit> edits, Stats st)
        {
            if (p1 <= p0) return;
            int n = p1 - p0;
            var texts = new string[n];
            var conv = new bool[n];
            bool any = false;
            for (int i = 0; i < n; i++) { texts[i] = pieces[p0 + i].Text; conv[i] = pieces[p0 + i].Convert; any |= conv[i]; }
            if (!any) return;
            st.Paragraphs++;
            string[] res = Native.ConvertRuns(texts, conv, mode.To, mode.Alphabet, mode.Flags);
            if (res == null) { st.Skipped++; return; }
            for (int i = 0; i < n; i++)
            {
                string old = texts[i], neu = res[i];
                if (!conv[i] || old == neu) continue;
                // Keep the unchanged head and tail (spaces, digits, punctuation) out of the replaced range.
                int head = 0, tail = 0;
                while (head < old.Length && head < neu.Length && old[head] == neu[head]) head++;
                while (tail < old.Length - head && tail < neu.Length - head && old[old.Length - 1 - tail] == neu[neu.Length - 1 - tail]) tail++;
                var pc = pieces[p0 + i];
                edits.Add(new Edit { Start = pc.Start + head, End = pc.End - tail, Text = neu.Substring(head, neu.Length - head - tail) });
            }
        }
    }
}
