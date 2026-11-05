// PowerPoint (docs/ms-addin/PLAN.md section 4.3): every text frame through Text2 (runs keep their formatting):
// shapes, groups, tables, SmartArt nodes, notes; slide masters and layouts only when asked. Changes are recorded in a
// Snapshot for "Undo transliteration"; Application.StartNewUndoEntry (2010+) also opens one native undo entry.
using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace UzTranslit.Office
{
    static class PowerPointAdapter
    {
        const int ppSelectionSlides = 1, ppSelectionShapes = 2, ppSelectionText = 3;

        /// scope: "selection" (the window's selection; nothing selected = the whole presentation), "presentation",
        /// "presentation+masters".
        public static Stats Convert(dynamic app, Mode mode, string scope, Snapshot snap)
        {
            if (app.Presentations.Count == 0) throw new UserMessage("msg.no_document");
            var st = new Stats();
            var clock = Stopwatch.StartNew();
            dynamic pres = Presentation(app);
            snap.Doc = pres.FullName;
            var ranges = new List<Func<dynamic>>();
            bool done = false;
            if (scope == "selection")
            {
                dynamic sel = null;
                int type = 0;
                try { sel = app.ActiveWindow.Selection; type = sel.Type; } catch { }
                if (type == ppSelectionText)
                {
                    dynamic tr = sel.TextRange2;
                    ranges.Add(() => tr);
                    done = true;
                }
                else if (type == ppSelectionShapes)
                {
                    foreach (dynamic s in sel.ShapeRange) Text2.Shapes(s, ranges);
                    done = true;
                }
                else if (type == ppSelectionSlides)
                {
                    foreach (dynamic sl in sel.SlideRange) Slide(sl, ranges);
                    done = true;
                }
            }
            if (!done)
            {
                foreach (dynamic sl in pres.Slides) Slide(sl, ranges);
                if (scope == "presentation+masters")
                {
                    foreach (dynamic d in pres.Designs)
                    {
                        dynamic master = d.SlideMaster;
                        foreach (dynamic s in master.Shapes) Text2.Shapes(s, ranges);
                        foreach (dynamic layout in master.CustomLayouts)
                            foreach (dynamic s in layout.Shapes) Text2.Shapes(s, ranges);
                    }
                }
            }
            try { app.StartNewUndoEntry(); } catch { }
            foreach (var r in ranges)
            {
                try
                {
                    var t = Text2.Convert(r, mode, st);
                    if (t.Edits.Count > 0) snap.Texts.Add(t);
                }
                catch (Exception e) { st.Skipped++; st.Error = e.Message; }
            }
            st.Stories = ranges.Count;
            st.Ms = clock.ElapsedMilliseconds;
            return st;
        }

        /// The active presentation; one opened without a window (automation) is never active: then the last one.
        public static dynamic Presentation(dynamic app)
        {
            try { return app.ActivePresentation; } catch { return app.Presentations[(int)app.Presentations.Count]; }
        }

        static void Slide(dynamic slide, List<Func<dynamic>> ranges)
        {
            foreach (dynamic s in slide.Shapes) Text2.Shapes(s, ranges);
            bool notes = true;
            try { notes = (int)slide.HasNotesPage != 0; } catch { }   // HasNotesPage is not in every version: then try
            try
            {
                if (notes)
                    foreach (dynamic s in slide.NotesPage.Shapes)
                    {
                        // Notes body placeholder (ppPlaceholderBody = 2); the slide image placeholder has no text.
                        try { if ((int)s.PlaceholderFormat.Type != 2) continue; } catch { continue; }
                        Text2.Shapes(s, ranges);
                    }
            }
            catch { }
        }

        public static int[] Restore(Snapshot snap)
        {
            int ok = 0, skip = 0;
            foreach (var t in snap.Texts) { var r = Text2.Restore(t); ok += r[0]; skip += r[1]; }
            return new[] { ok, skip };
        }
    }
}
