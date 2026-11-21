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
// Cost is Word calls (0.1-5 ms each), so a story's text is read once: a story with no letter, or one that converts to
// itself, is never walked run by run. Stories are walked one at a time and every Range is released after use: Word
// updates every live Range on each edit (a document of 4,125 text boxes took 170 s in 0.2.0, 6 s now).
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace UzTranslit.Office
{
    sealed class Stats
    {
        public int Paragraphs, Pieces, Edits, Skipped, Stories, XmlParagraphs, XmlInserts, XmlRefused, XmlMismatch, Formulas, Rich, Prefixed;
        public string Refused = "", Error = "";
        public long Ms;
        /// Word: whether the document has any bookmark or comment at all (asked once; per story they cost ~2 ms each).
        public bool AnyBookmarks = true, AnyComments = true;
        public override string ToString()
        {
            return "paragraphs=" + Paragraphs + ";pieces=" + Pieces + ";edits=" + Edits + ";skipped=" + Skipped
                + ";stories=" + Stories + ";xml_paragraphs=" + XmlParagraphs + ";xml_inserts=" + XmlInserts + ";xml_refused=" + XmlRefused + ";xml_mismatch=" + XmlMismatch + (Refused.Length > 0 ? "(" + Refused + ")" : "") + ";formulas=" + Formulas + ";rich=" + Rich + ";prefixed=" + Prefixed + ";ms=" + Ms + (Error.Length > 0 ? ";last_error=" + Error : "");
        }
    }

    /// A long command runs on Word's UI thread, so Word paints nothing and Windows marks its window "Not Responding"
    /// after 5 s without a look at the message queue. Every 250 ms (after the first second) this peeks at the queue
    /// without removing anything (no click or key is handled mid-command) and shows in the status bar how many
    /// paragraphs are done (a count, not a percent: the stories are walked one by one, their number is not known).
    sealed class Progress
    {
        readonly dynamic app;
        readonly string text;
        readonly Stopwatch clock = Stopwatch.StartNew();
        long next = 1000;
        bool shown;
        /// Paragraphs read so far.
        public long Done;

        public Progress(dynamic app, string text) { this.app = app; this.text = text; }

        public void Tick(long part)
        {
            if (clock.ElapsedMilliseconds < next) return;
            next = clock.ElapsedMilliseconds + 250;
            Ui.Peek();
            if (text == null) return;
            try { app.StatusBar = text.Replace("{n}", (Done + part).ToString()); shown = true; } catch { }
        }

        public void Clear() { if (shown) try { app.StatusBar = ""; } catch { } }
    }

    static class WordAdapter
    {
        const int wdCharacterFormatting = 13, wdNoProtection = -1, wdMainTextStory = 1;
        /// Paragraphs per WordOpenXML read on the fast path.
        const int XmlGroup = 50;
        /// The running command's progress (Word calls the add-in on its one UI thread).
        static Progress progress;

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
        /// progressText: the status bar text of a long command, "{n}" = paragraphs done (null: no status bar text).
        public static Stats Convert(dynamic app, Mode mode, bool wholeDocument, string undoLabel, bool fast, string progressText)
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
            progress = new Progress((object)app, progressText);

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
            // Document-wide: Bookmarks covers every story (text boxes included, measured), Comments every comment.
            try { st.AnyBookmarks = (int)bookmarks.Count > 0; st.AnyComments = (int)doc.Comments.Count > 0; } catch { }
            try
            {
                // Our own methods get a Range as object: a call with a dynamic argument is bound at run time, which costs
                // ~0.1-1 ms per call for a COM object (measured), more than most Word calls.
                foreach (object s in whole ? Stories((object)doc) : new object[] { sel.Range })
                {
                    st.Stories++;
                    dynamic d = s;
                    int from = d.Start, to = d.End;
                    if (xml && (int)d.StoryType == wdMainTextStory) Main(s, from, to, (int)doc.Content.End, mode, st);
                    else PerRun(s, from, to, mode, st, true);
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
                progress.Clear();
                progress = null;
            }
            st.Ms = clock.ElapsedMilliseconds;
            return st;
        }

        /// Every story, one at a time, each Range released once it is done. Word keeps every live Range object up to
        /// date on each edit: holding one per story (4,000+ text boxes in a real document) made every write slower
        /// than the one before (1.6 ms at the start, 8.7 ms after 4,000; flat at 1-2 ms with the ranges released).
        static IEnumerable<object> Stories(dynamic doc)
        {
            foreach (object first in doc.StoryRanges)
                for (object r = first; r != null; )
                {
                    yield return r;
                    object next = ((dynamic)r).NextStoryRange;
                    Free(r);
                    r = next;
                }
            // Text boxes in headers and footers are not on the NextStoryRange chain. Converting a story twice is
            // harmless: the conversion is idempotent.
            var boxes = new List<object>();
            try
            {
                foreach (dynamic shp in doc.Sections[1].Headers[1].Shapes)
                    try { if (shp.TextFrame.HasText != 0) boxes.Add((object)shp.TextFrame.TextRange); } catch { }
            }
            catch { }
            foreach (object b in boxes) { yield return b; Free(b); }
        }

        /// Releases a COM object now instead of at some later garbage collection (see Stories).
        static void Free(object o)
        {
            if (o != null && Marshal.IsComObject(o)) Marshal.ReleaseComObject(o);
        }

        /// The main text: whole paragraphs inside [from, to) except the story's last one go through WordXml in groups;
        /// the rest (partial paragraphs at the selection edges, the last paragraph, non-plain paragraphs) per run.
        static void Main(dynamic story, int from, int to, int storyEnd, Mode mode, Stats st)
        {
            dynamic rng = story.Duplicate;
            rng.SetRange(from, to);
            var ps = new List<int[]>();
            // [start, end, in a table]: a cell's paragraph mark is the cell mark, never written through InsertXML.
            foreach (dynamic p in rng.Paragraphs) { dynamic pr = p.Range; ps.Add(new[] { (int)pr.Start, (int)pr.End, (bool)pr.Information[12] ? 1 : 0 }); Free((object)pr); Free((object)p); }
            // Segments, last to first: [i, j) of whole candidate paragraphs (chunked), or a per-run span.
            int k = ps.Count;
            while (k > 0)
            {
                int j = k;
                bool cand = Candidate(ps[j - 1], from, to, storyEnd);
                while (k > 0 && Candidate(ps[k - 1], from, to, storyEnd) == cand && (!cand || j - k < XmlGroup)) k--;
                if (cand) XmlParagraphs((object)story, ps, k, j, mode, st);
                else PerRun((object)story, Math.Max(ps[k][0], from), Math.Min(ps[j - 1][1], to), mode, st);
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
            progress.Tick(0);
            dynamic r = story.Duplicate;
            // Every XML range ends just before the last paragraph mark (WordXml.Convert says why).
            r.SetRange(ps[i][0], ps[j - 1][1] - 1);
            var x = WordXml.Load((string)r.WordOpenXML);
            bool[] plain = WordXml.Classify(x, j - i);
            if (plain == null)
            {
                if (j - i == 1) { PerRun((object)story, ps[i][0], ps[i][1], mode, st); return; }
                int m = (i + j) / 2;
                XmlParagraphs((object)story, ps, m, j, mode, st);
                XmlParagraphs((object)story, ps, i, m, mode, st);
                return;
            }
            int k = j;
            while (k > i)
            {
                int e = k;
                bool c = plain[k - 1 - i];
                while (k > i && plain[k - 1 - i] == c) k--;
                if (!c) { PerRun((object)story, ps[k][0], ps[e - 1][1], mode, st); continue; }
                int s = ps[k][0], end = ps[e - 1][1] - 1;
                r.SetRange(s, end);
                string old = r.Text;
                // The exact span: a package cut out of a larger read was refused by InsertXML (0x800A1801) in tests.
                var sub = k == i && e == j ? x : WordXml.Load((string)r.WordOpenXML);
                string want, source;
                string pkg = WordXml.Convert(sub, e - k, mode, out want, out source);
                // The package must hold exactly the text Word shows, or what it lost would be lost for good.
                if (pkg == null || source != old) { st.XmlMismatch++; PerRun((object)story, ps[k][0], ps[e - 1][1], mode, st); continue; }
                if (old == want) { st.Paragraphs += e - k; continue; }   // nothing changes
                try { r.InsertXML(pkg); }
                catch (System.Runtime.InteropServices.COMException ex)
                {
                    // Word refused this location: if nothing changed, convert it per run instead.
                    r.SetRange(s, end);
                    if ((string)r.Text != old) throw;
                    st.XmlRefused++;
                    st.Refused = "0x" + ex.ErrorCode.ToString("X8") + "@" + ps[k][0];
                    PerRun((object)story, ps[k][0], ps[e - 1][1], mode, st);
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
        /// flat: every position of the range reads as one character and none is a field or cell mark, so it holds no
        /// field and no content control (both have marks that Range.Text leaves out, measured): those are not asked.
        static void Boundaries(dynamic range, SortedSet<int> cuts, List<int[]> locked, bool flat, Stats st)
        {
            if (!flat)
                foreach (dynamic f in range.Fields)
                {
                    dynamic code = f.Code, res = f.Result;
                    int cs = code.Start, ce = code.End, rs = res.Start, re = res.End;
                    Free((object)code); Free((object)res); Free((object)f);
                    if (rs >= ce) { locked.Add(new[] { cs - 1, rs }); locked.Add(new[] { re, re + 1 }); }
                    else locked.Add(new[] { cs - 1, ce + 1 });
                }
            if (st.AnyBookmarks) foreach (dynamic b in range.Bookmarks) { cuts.Add((int)b.Start); cuts.Add((int)b.End); Free((object)b); }
            if (st.AnyComments) try { foreach (dynamic c in range.Comments) { dynamic sc = c.Scope; cuts.Add((int)sc.Start); cuts.Add((int)sc.End); Free((object)sc); } } catch { }
            if (!flat)
                try
                {
                    foreach (dynamic cc in range.ContentControls)
                    {
                        dynamic r = cc.Range;
                        int a = r.Start, b = r.End;
                        locked.Add(new[] { a - 1, a });
                        locked.Add(new[] { b, b + 1 });
                        Free((object)r);
                    }
                }
                catch { }
            // Equations: variable names and operators are not Uzbek text. Asked in every story (an equation in a text box
            // is in neither Document.OMaths nor missing from Range.Text, measured); Count first is one call fewer.
            try
            {
                dynamic oms = range.OMaths;
                if ((int)oms.Count > 0)
                    foreach (dynamic m in oms) { dynamic r = m.Range; locked.Add(new[] { (int)r.Start, (int)r.End }); Free((object)r); Free((object)m); }
                Free((object)oms);
            }
            catch { }
            foreach (var l in locked) { cuts.Add(l[0]); cuts.Add(l[1]); }
        }

        static bool IsLocked(List<int[]> locked, int a, int b)
        {
            foreach (var l in locked) if (a >= l[0] && b <= l[1]) return true;
            return false;
        }

        /// own: `story` is exactly [from, to) and not needed afterwards, so it is used as the working range (saves a
        /// Duplicate and a SetRange per story: ~1 ms per text box).
        static void PerRun(dynamic story, int from, int to, Mode mode, Stats st, bool own = false)
        {
            if (to <= from) return;
            dynamic r = story;
            if (!own) { r = story.Duplicate; r.SetRange(from, to); }
            var cuts = new SortedSet<int>();
            var locked = new List<int[]>();
            // One read of the whole span: when it is flat (see Boundaries) the pieces are cut out of this text instead
            // of one Range.Text read each, and a span that converts to itself is not walked run by run at all.
            string all = r.Text ?? "";
            // No letter, nothing to convert (the engine rewrites only letters and the apostrophes between them): e.g. a
            // main text made only of text-box anchors and section breaks, which was walked position by position.
            if (!HasLetter(all)) { progress.Done += MarkCount(all); progress.Tick(0); Done((object)r, own); return; }
            string flat = all.Length == to - from && all.IndexOfAny(Marks) < 0 ? all : null;
            Boundaries((object)r, cuts, locked, flat != null, st);
            if (flat != null)
            {
                // Cutting at runs does not change the conversion (uzt_convert_runs is split-invariant), so the whole
                // span cut only at the boundaries gives the same edits as the run walk would: none = nothing to do.
                var whole = new List<Piece>();
                int a0 = from;
                foreach (int c in cuts) if (c > from && c < to) { Read((object)r, a0, c, locked, whole, st, flat, from); a0 = c; }
                Read((object)r, a0, to, locked, whole, st, flat, from);
                var probe = new Stats();
                if (Edits(whole, mode, probe).Count == 0)
                {
                    st.Paragraphs += probe.Paragraphs; st.Skipped += probe.Skipped; st.Pieces += whole.Count;
                    progress.Done += MarkCount(all);
                    progress.Tick(0);
                    Done((object)r, own);
                    return;
                }
            }

            var pieces = new List<Piece>();
            int pos = from, marks = MarkCount(all);
            while (pos < to)
            {
                int end = pos + 1;
                // A control character (paragraph mark...) is never edited: its formatting run is not needed.
                if (flat == null || flat[pos - from] >= ' ' || flat[pos - from] == '\t')
                {
                    r.SetRange(pos, pos);
                    r.MoveEnd(wdCharacterFormatting, 1);
                    end = Math.Min(Math.Max((int)r.End, pos + 1), to);
                }
                // Cut [pos, end) at the boundaries inside it.
                int a = pos;
                if (end - pos > 1)
                    foreach (int c in cuts.GetViewBetween(pos + 1, end - 1)) { Read((object)r, a, c, locked, pieces, st, flat, from); a = c; }
                Read((object)r, a, end, locked, pieces, st, flat, from);
                pos = end;
                progress.Tick((long)marks * (pos - from) / (to - from));
            }
            progress.Done += marks;

            var edits = Edits(pieces, mode, st);
            st.Pieces += pieces.Count;

            dynamic w = r;
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
                progress.Tick(0);
            }
            Done((object)r, own);
        }

        /// The working range of PerRun: released unless it is the caller's story range (Stories releases that).
        static void Done(object r, bool own) { if (!own) Free(r); }

        /// Paragraph, cell and page marks in a text (for the progress count).
        static int MarkCount(string s)
        {
            int n = 0;
            foreach (char c in s) if (c == '\r' || c == '\a' || c == '\f') n++;
            return n;
        }

        static bool HasLetter(string s)
        {
            foreach (char c in s) if (char.IsLetter(c)) return true;
            return false;
        }

        static readonly char[] Marks = { '\a', '\x13', '\x14', '\x15' };

        /// Groups the pieces into paragraphs, converts them, returns the edits.
        static List<Edit> Edits(List<Piece> pieces, Mode mode, Stats st)
        {
            var edits = new List<Edit>();
            int p0 = 0;
            for (int i = 0; i <= pieces.Count; i++)
            {
                if (i < pieces.Count && !pieces[i].ParagraphEnd) continue;
                int p1 = i < pieces.Count ? i + 1 : i;
                ConvertParagraph(pieces, p0, p1, mode, edits, st);
                p0 = p1;
            }
            return edits;
        }

        /// One formatting-uniform span [a, b) -> pieces. Locked or unreadable spans become verbatim pieces. `flat`: the
        /// text of the whole span from `origin`, cut here instead of read (null: read through Range.Text).
        static void Read(dynamic r, int a, int b, List<int[]> locked, List<Piece> pieces, Stats st, string flat, int origin)
        {
            if (b <= a) return;
            if (IsLocked(locked, a, b)) { pieces.Add(new Piece { Start = a, End = b, Text = "" }); return; }
            string t;
            if (flat != null) t = flat.Substring(a - origin, b - a);
            else { r.SetRange(a, b); t = r.Text ?? ""; }
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
