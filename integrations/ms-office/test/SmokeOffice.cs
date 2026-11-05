// Smoke test of the Excel and PowerPoint adapters (docs/ms-addin/PLAN.md section 7). Same rules as Smoke.cs: its own
// Office instance, its own files under <test-docs>, the add-in registered only for the run (run-smoke.ps1).
// Before quitting, an instance that holds any workbook/presentation this test did not create (a user's file that
// ended up in it) is NOT quit: it is made visible and handed to the user.
//
//   smoke-office.exe excel|powerpoint <test-docs dir> [--big ROWS]     exit 0 = every check passed
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;

static class SmokeOffice
{
    static int fails;
    static dynamic addin;

    static void Check(bool ok, string what)
    {
        Console.WriteLine((ok ? "PASS " : "FAIL ") + what);
        if (!ok) fails++;
    }

    static string Vis(object o)
    {
        string s = o as string;
        if (s == null) return o == null ? "(null)" : "(" + o.GetType().Name + ")" + o;
        return string.Concat(s.Select(c => c < ' ' ? "<" + (int)c + ">" : c.ToString()));
    }

    static string Expected(string text, string mode)
    {
        var parts = Regex.Split(text, "([\r\v\n])");
        return string.Concat(parts.Select(p => p.Length == 1 && p[0] < ' ' ? p : (string)addin.ConvertText(p, mode)));
    }

    static dynamic Connect(dynamic app)
    {
        dynamic ca = null;
        foreach (dynamic c in app.COMAddIns) if ((string)c.ProgId == "UzTranslit.OfficeAddin") ca = c;
        if (ca == null) throw new Exception("add-in not registered");
        bool atStart = ca.Connect;
        if (!atStart) ca.Connect = true;
        Console.WriteLine("add-in loaded at start: " + atStart + "; connected: " + ca.Connect);
        return ca.Object;
    }

    // ---------------- Excel ----------------

    static string BoldMap(dynamic cell)
    {
        string t = cell.Value2;
        var sb = new System.Text.StringBuilder();
        for (int i = 1; i <= t.Length; i++) sb.Append((bool)cell.Characters[i, 1].Font.Bold ? 'B' : '.');
        return sb.ToString();
    }

    static string ShapeMap(dynamic tr)
    {
        // Bold (B) / italic (I) / plain (.) per character of a TextRange2.
        var sb = new System.Text.StringBuilder();
        int n = tr.Length;
        for (int i = 1; i <= n; i++)
        {
            dynamic f = tr.Characters[i, 1].Font;
            sb.Append((int)f.Bold != 0 ? 'B' : (int)f.Italic != 0 ? 'I' : '.');
        }
        return sb.ToString();
    }

    static Dictionary<string, object> Values(dynamic ws, string addr)
    {
        var d = new Dictionary<string, object>();
        foreach (dynamic c in ws.Range[addr].Cells) d[(string)c.Address] = c.Value2;
        return d;
    }

    static void Excel(dynamic x, string dir, int bigRows, List<string> mine)
    {
        dynamic wb = x.Workbooks.Add();
        dynamic ws = wb.Worksheets[1];
        dynamic ws2 = wb.Worksheets.Add(After: ws);
        ws.Activate();
        ws.Cells[1, 1].Value2 = "Shahar markazida yangi maktab";
        ws.Cells[1, 2].Value2 = "olma";
        ws.Cells[1, 3].Value2 = "nok";
        ws.Cells[2, 1].Value2 = "Toshkent shahri";
        ws.Cells[2, 1].Characters[1, 3].Font.Bold = true;       // rich text, bold ends mid-word: Tos|hkent
        ws.Cells[3, 1].Formula = "=\"choy \"&A1";               // a formula with a text literal: never touched
        ws.Cells[4, 1].Value2 = 42;
        ws.Cells[5, 1].Value2 = 45000; ws.Cells[5, 1].NumberFormat = "dd.mm.yyyy";
        ws.Cells[6, 1].Value2 = "'=bu matn";                    // text that looks like a formula
        ws.Cells[7, 1].Value2 = "birinchi qator\nikkinchi qator";
        ws.Cells[2, 2].AddComment("Izoh: muhim hujjat");
        dynamic tb = ws.Shapes.AddTextbox(1, 250, 20, 160, 50);
        tb.TextFrame2.TextRange.Text = "Matn qutisi ichida";
        tb.TextFrame2.TextRange.Characters[1, 4].Font.Bold = -1;
        ws2.Cells[1, 1].Value2 = "Май 5";                       // -> "May 5": Excel would read a date
        ws2.Cells[2, 1].Value2 = "Ўзбекистон";
        string path = Path.Combine(dir, "features.xlsx");
        if (File.Exists(path)) File.Delete(path);
        wb.SaveAs(path, 51);
        mine.Add(path);

        string[] addrs = { "$A$1", "$B$1", "$C$1", "$A$2", "$A$6", "$A$7" };
        Dictionary<string, object> before = Values(ws, "A1:C7");
        string formula = ws.Cells[3, 1].Formula, bold0 = BoldMap(ws.Cells[2, 1]);
        string tb0 = tb.TextFrame2.TextRange.Text, tbMap0 = ShapeMap(tb.TextFrame2.TextRange);
        string note0 = ws.Cells[2, 2].Comment.Text();
        Dictionary<string, object> sheet2 = Values(ws2, "A1:A2");

        string st = addin.ConvertIn("cyrl", "sheet");
        Console.WriteLine("excel cyrl sheet: " + st);
        foreach (var a in addrs)
        {
            object now = ws.Range[a].Value2;
            string want = Expected((string)before[a], "cyrl");
            Check(now as string == want, "excel cyrl " + a + ": " + Vis(now) + (now as string == want ? "" : " want " + Vis(want)));
        }
        Check((string)ws.Cells[3, 1].Formula == formula, "excel formula unchanged: " + (string)ws.Cells[3, 1].Formula);
        Check(st.Contains("formulas=1;"), "excel the formula cell was counted as left unchanged");
        Check((double)ws.Cells[4, 1].Value2 == 42 && (double)ws.Cells[5, 1].Value2 == 45000, "excel numbers and dates unchanged");
        Check((bool)ws.Cells[6, 1].HasFormula == false, "excel '=...' text stays text");
        string bold1 = BoldMap(ws.Cells[2, 1]);
        Check(bold1.StartsWith("BBB") && bold1.Substring(3).All(c => c == '.'), "excel rich text cell keeps its bold run: " + bold0 + " -> " + bold1);
        string tb1 = tb.TextFrame2.TextRange.Text;
        Check(tb1 == Expected(tb0, "cyrl"), "excel text box: " + Vis(tb1));
        Check(ShapeMap(tb.TextFrame2.TextRange) == tbMap0.Substring(0, Math.Min(tbMap0.Length, tb1.Length)) || ShapeMap(tb.TextFrame2.TextRange).StartsWith("BBBB."), "excel text box keeps its bold run: " + tbMap0 + " -> " + ShapeMap(tb.TextFrame2.TextRange));
        string note1 = ws.Cells[2, 2].Comment.Text();
        Check(note1 == Expected(note0, "cyrl"), "excel note: " + Vis(note1));
        Dictionary<string, object> sheet2b = Values(ws2, "A1:A2");
        Check(sheet2b.All(kv => Equals(kv.Value, sheet2[kv.Key])), "excel scope sheet: the other sheet untouched");

        string u = addin.UndoLast();
        Console.WriteLine("excel undo: " + u);
        Dictionary<string, object> after = Values(ws, "A1:C7");
        Check(addrs.All(a => Equals(after[a], before[a])), "excel Undo transliteration restores every cell");
        Check(BoldMap(ws.Cells[2, 1]) == bold0, "excel Undo restores the rich text runs: " + BoldMap(ws.Cells[2, 1]));
        Check((string)tb.TextFrame2.TextRange.Text == tb0 && ShapeMap(tb.TextFrame2.TextRange) == tbMap0, "excel Undo restores the text box and its runs");
        Check((string)ws.Cells[2, 2].Comment.Text() == note0, "excel Undo restores the note");
        Check((string)addin.UndoLast() == "nothing", "excel a second Undo has nothing to undo");

        // An edit after the conversion is kept by Undo.
        addin.ConvertIn("cyrl", "sheet");
        ws.Cells[1, 2].Value2 = "qo'lda tahrir";
        string u2 = addin.UndoLast();
        Check(u2.Contains("skipped=1") && (string)ws.Cells[1, 2].Value2 == "qo'lda tahrir" && Equals(ws.Cells[1, 1].Value2, before["$A$1"]),
            "excel Undo leaves a cell edited after the conversion: " + u2);
        ws.Cells[1, 2].Value2 = "olma";

        // Whole workbook, to Latin: "Май 5" must stay text, not become a date.
        string st2 = addin.ConvertIn("latn", "workbook");
        Console.WriteLine("excel latn workbook: " + st2);
        object may = ws2.Cells[1, 1].Value2;
        Check(may as string == "May 5", "excel 'Май 5' -> text 'May 5', not a date: " + Vis(may));
        Check(!st2.Contains("prefixed=0;"), "excel a text Excel would parse was written with the text prefix");
        Check((string)ws2.Cells[2, 1].Value2 == "Özbekiston", "excel workbook scope reaches the second sheet");
        addin.UndoLast();
        Check((string)ws2.Cells[1, 1].Value2 == "Май 5", "excel Undo restores the second sheet");

        // Selection.
        ws.Range["A1:A2"].Select();
        string st3 = addin.ConvertIn("cyrl", "selection");
        Check((string)ws.Cells[1, 1].Value2 == Expected((string)before["$A$1"], "cyrl") && (string)ws.Cells[1, 2].Value2 == "olma",
            "excel selection A1:A2 only: " + st3);
        addin.UndoLast();

        if (bigRows > 0)
        {
            // Timing: bigRows x 20 text cells, a formula column, every 10th row with a bold first word.
            dynamic ws3 = wb.Worksheets.Add();
            var words = new[] { "Shahar", "markazida", "yangi", "maktab", "qurildi", "choy", "gʻalla", "shoir", "Toshkent", "Oʻzbekiston" };
            var arr = new object[bigRows, 20];
            var rnd = new Random(7);
            for (int r = 0; r < bigRows; r++) for (int c = 0; c < 20; c++) arr[r, c] = words[rnd.Next(10)] + " " + words[rnd.Next(10)] + " " + words[rnd.Next(10)];
            ws3.Range[ws3.Cells[1, 1], ws3.Cells[bigRows, 20]].Value2 = arr;
            ws3.Range[ws3.Cells[1, 21], ws3.Cells[bigRows, 21]].Formula = "=LEN(A1)";
            for (int r = 1; r <= bigRows; r += 10) ws3.Cells[r, 1].Characters[1, 3].Font.Bold = true;
            object[,] v0 = ws3.UsedRange.Value2;
            var sw = Stopwatch.StartNew();
            string sb = addin.ConvertIn("cyrl", "sheet");
            Console.WriteLine("excel big (" + bigRows + " x 20 text cells, " + (bigRows / 10) + " rich): " + sb + " (wall " + sw.ElapsedMilliseconds + " ms)");
            object[,] v1 = ws3.UsedRange.Value2;
            bool ok = true;
            for (int r = 1; r <= bigRows && ok; r++) for (int c = 1; c <= 20 && ok; c++) ok = (string)v1[r, c] == Expected((string)v0[r, c], "cyrl");
            Check(ok, "excel big: every cell equals the engine's conversion");
            Check(Equals(ws3.Cells[1, 21].Formula, "=LEN(A1)"), "excel big: formulas unchanged");
            sw.Restart();
            string ub = addin.UndoLast();
            Console.WriteLine("excel big undo: " + ub + " (" + sw.ElapsedMilliseconds + " ms)");
            object[,] v2 = ws3.UsedRange.Value2;
            ok = true;
            for (int r = 1; r <= bigRows && ok; r++) for (int c = 1; c <= 20 && ok; c++) ok = Equals(v2[r, c], v0[r, c]);
            Check(ok, "excel big: Undo transliteration restores every cell");
        }
        wb.Close(false);
    }

    // ---------------- PowerPoint ----------------

    static List<KeyValuePair<string, dynamic>> PptTexts(dynamic pres, bool masters)
    {
        var l = new List<KeyValuePair<string, dynamic>>();
        Action<string, dynamic> shape = null;
        shape = (p, s) =>
        {
            int type = s.Type;
            if (type == 6) { int k = 0; foreach (dynamic g in s.GroupItems) shape(p + "/g" + k++, g); return; }
            if ((int)s.HasTable != 0)
            {
                dynamic t = s.Table;
                for (int r = 1; r <= (int)t.Rows.Count; r++) for (int c = 1; c <= (int)t.Columns.Count; c++)
                        l.Add(new KeyValuePair<string, dynamic>(p + "/cell" + r + c, t.Cell(r, c).Shape.TextFrame2.TextRange));
                return;
            }
            if ((int)s.HasSmartArt != 0)
            {
                dynamic nodes = s.SmartArt.AllNodes;
                for (int i = 1; i <= (int)nodes.Count; i++) l.Add(new KeyValuePair<string, dynamic>(p + "/node" + i, nodes.Item(i).TextFrame2.TextRange));
                return;
            }
            if ((int)s.HasTextFrame != 0 && (int)s.TextFrame2.HasText != 0) l.Add(new KeyValuePair<string, dynamic>(p, s.TextFrame2.TextRange));
        };
        foreach (dynamic sl in pres.Slides)
        {
            foreach (dynamic s in sl.Shapes) shape("slide" + sl.SlideIndex + "/" + s.Name, s);
            foreach (dynamic s in sl.NotesPage.Shapes)
                if ((int)s.Type == 14 && (int)s.PlaceholderFormat.Type == 2) shape("notes" + sl.SlideIndex, s);
        }
        if (masters) foreach (dynamic s in pres.SlideMaster.Shapes) shape("master/" + s.Name, s);
        return l;
    }

    static Dictionary<string, string[]> Snap(dynamic pres, bool masters)
    {
        // text and per-character bold/italic map of every text range
        List<KeyValuePair<string, dynamic>> l = PptTexts(pres, masters);
        return l.ToDictionary(kv => kv.Key, kv => new string[] { (string)kv.Value.Text, (string)ShapeMap(kv.Value) });
    }

    static void PowerPoint(dynamic p, string dir, List<string> mine)
    {
        dynamic pres = p.Presentations.Add(0);           // msoFalse: no window
        dynamic slide = pres.Slides.Add(1, 2);           // ppLayoutText: title + body
        slide.Shapes[1].TextFrame2.TextRange.Text = "Yangi maktab qurildi";
        dynamic body = slide.Shapes[2].TextFrame2.TextRange;
        body.Text = "Shahar markazida bayram\rChoy va non, gʻalla";
        body.Characters[1, 3].Font.Bold = -1;            // Sha|har: bold ends mid-word
        dynamic tb = slide.Shapes.AddTextbox(1, 50, 380, 300, 40);
        tb.TextFrame2.TextRange.Text = "Matn qutisi ichida";
        tb.TextFrame2.TextRange.Characters[1, 4].Font.Italic = -1;
        dynamic tbl = slide.Shapes.AddTable(2, 2, 400, 380, 200, 80);
        tbl.Table.Cell(1, 1).Shape.TextFrame2.TextRange.Text = "olma";
        tbl.Table.Cell(1, 2).Shape.TextFrame2.TextRange.Text = "nok";
        tbl.Table.Cell(2, 1).Shape.TextFrame2.TextRange.Text = "uzum";
        tbl.Table.Cell(2, 2).Shape.TextFrame2.TextRange.Text = "anor";
        dynamic g1 = slide.Shapes.AddTextbox(1, 50, 450, 100, 30); g1.TextFrame2.TextRange.Text = "birinchi";
        dynamic g2 = slide.Shapes.AddTextbox(1, 200, 450, 100, 30); g2.TextFrame2.TextRange.Text = "ikkinchi";
        slide.Shapes.Range(new object[] { (string)g1.Name, (string)g2.Name }).Group();
        bool smart = false;
        try
        {
            dynamic sa = slide.Shapes.AddSmartArt(p.SmartArtLayouts[1], 500, 20, 200, 150);
            dynamic nodes = sa.SmartArt.AllNodes;
            for (int i = 1; i <= (int)nodes.Count; i++) nodes.Item(i).TextFrame2.TextRange.Text = "tugun " + i;
            smart = true;
        }
        catch (Exception e) { Console.WriteLine("SmartArt not created here: " + e.Message.Split('\n')[0]); }
        foreach (dynamic s in slide.NotesPage.Shapes)
            if ((int)s.Type == 14 && (int)s.PlaceholderFormat.Type == 2) s.TextFrame2.TextRange.Text = "Maʼruzachi uchun izoh";
        string path = Path.Combine(dir, "features.pptx");
        if (File.Exists(path)) File.Delete(path);
        pres.SaveAs(path, 24);
        mine.Add(path);

        Dictionary<string, string[]> s0 = Snap(pres, true);
        Console.WriteLine("powerpoint: " + s0.Count + " text ranges (incl. master), SmartArt " + (smart ? "yes" : "no"));
        string st = addin.ConvertIn("cyrl", "presentation");
        Console.WriteLine("powerpoint cyrl presentation: " + st);
        Dictionary<string, string[]> s1 = Snap(pres, true);
        foreach (var k in s0.Keys)
        {
            bool master = k.StartsWith("master/");
            string want = master ? s0[k][0] : Expected(s0[k][0], "cyrl");
            Check(s1[k][0] == want, "powerpoint " + k + (master ? " (master, not in scope) unchanged" : " converted") + (s1[k][0] == want ? "" : ": " + Vis(s1[k][0]) + " want " + Vis(want)));
        }
        string bodyKey = s0.Keys.First(k => k.StartsWith("slide1/") && s0[k][0].StartsWith("Shahar"));
        int nb = ((string)addin.ConvertText("Sha", "cyrl")).Length;   // "Sha" -> "Ша": sh is one letter
        Check(s1[bodyKey][1].StartsWith(new string('B', nb)) && !s1[bodyKey][1].Substring(nb).Contains('B'), "powerpoint mid-word bold run kept: " + s0[bodyKey][1].Substring(0, 8) + " -> " + s1[bodyKey][1].Substring(0, 8));
        string tbKey = s0.Keys.First(k => s0[k][0] == "Matn qutisi ichida");
        Check(s1[tbKey][1].StartsWith("IIII") && !s1[tbKey][1].Substring(4).Contains('I'), "powerpoint italic run kept: " + s1[tbKey][1]);

        // Native undo: PowerPoint has no Undo in its object model; ExecuteMso("Undo") needs a window.
        string native;
        try { p.CommandBars.ExecuteMso("Undo"); Dictionary<string, string[]> sN = Snap(pres, false); native = sN.Keys.All(k => sN[k][0] == s0[k][0]) ? "one step restores all" : "one step does not restore all"; }
        catch (Exception e) { native = "not measurable here (" + e.Message.Split('\n')[0] + ")"; }
        Console.WriteLine("powerpoint native Undo: " + native);

        Dictionary<string, string[]> sBefore = Snap(pres, true);
        string u = addin.UndoLast();
        Console.WriteLine("powerpoint undo: " + u);
        Dictionary<string, string[]> s2 = Snap(pres, true);
        string bad = s0.Keys.FirstOrDefault(k => s2[k][0] != s0[k][0] || s2[k][1] != s0[k][1]);
        Check(bad == null || native.StartsWith("one step restores"), "powerpoint Undo transliteration restores every text and run" + (bad == null ? "" : ": " + bad + " " + Vis(s2[bad][0])));

        // Masters only when asked.
        string st2 = addin.ConvertIn("latn", "presentation+masters");
        addin.ConvertIn("cyrl", "presentation+masters");
        Dictionary<string, string[]> s3 = Snap(pres, true);
        string mk = s0.Keys.First(k => k.StartsWith("master/"));
        Check(s3[mk][0] == Expected(s0[mk][0], "cyrl"), "powerpoint masters converted when asked: " + Vis(s3[mk][0]));
        addin.UndoLast();
        pres.Close();
    }

    // ---------------- main ----------------

    static int Main(string[] args)
    {
        string app = args[0], dir = args[1];
        int big = args.Length > 3 && args[2] == "--big" ? int.Parse(args[3]) : 0;
        Directory.CreateDirectory(dir);
        var mine = new List<string>();
        string prog = app == "excel" ? "Excel.Application" : "PowerPoint.Application";
        string proc = app == "excel" ? "EXCEL" : "POWERPNT";
        if (app == "powerpoint" && Process.GetProcessesByName(proc).Length > 0)
        {
            // PowerPoint is single-instance: creating it would attach to the user's PowerPoint.
            Console.WriteLine("SKIPPED: PowerPoint is already running (not ours, not touched)");
            return 3;
        }
        int[] before = Process.GetProcessesByName(proc).Select(q => q.Id).ToArray();
        dynamic o = Activator.CreateInstance(Type.GetTypeFromProgID(prog));
        int[] started = Process.GetProcessesByName(proc).Select(q => q.Id).Where(id => !before.Contains(id)).ToArray();
        Console.WriteLine(app + " " + o.Version + " build " + o.Build + ", own process: " + string.Join(",", started));
        try
        {
            if (app == "excel") { o.DisplayAlerts = false; }
            addin = Connect(o);
            if (app == "excel") Excel(o, dir, big, mine); else PowerPoint(o, dir, mine);
        }
        catch (Exception e) { Console.WriteLine("FAIL exception: " + e); fails++; }
        finally
        {
            addin = null;   // a proxy to the add-in in the Office process: it alone keeps that process alive
            // Quit only if everything open is ours (this test closes its own files).
            bool foreign = false;
            try
            {
                dynamic docs = app == "excel" ? o.Workbooks : o.Presentations;
                foreach (dynamic d in docs) { string n = d.FullName; if (!mine.Contains(n)) foreign = true; else d.Close(); }
            }
            catch { }
            if (foreign)
            {
                Console.WriteLine("NOTE: the instance holds a file this test did not open: left running and made visible");
                try { o.Visible = true; if (app == "excel") o.UserControl = true; } catch { }
            }
            else
            {
                try { o.Quit(); } catch { }
                o = null;
                GC.Collect(); GC.WaitForPendingFinalizers();
                // The add-in must not keep the host alive (it once did: undo snapshots held cell references).
                var sw = Stopwatch.StartNew();
                bool gone = false;
                while (!gone && sw.ElapsedMilliseconds < 3000) { gone = started.All(id => { try { return Process.GetProcessById(id).HasExited; } catch { return true; } }); if (!gone) System.Threading.Thread.Sleep(250); }
                // This process's own proxies (dynamic call-site caches) can keep the host alive until it exits, so
                // run-smoke.ps1 checks the exit after this process ends; here it is only reported.
                Console.WriteLine(app + " process exited while the test still ran: " + gone);
                File.WriteAllText(Path.Combine(dir, "pids-" + app + ".txt"), string.Join(",", started));
            }
        }
        Console.WriteLine(fails == 0 ? "SMOKE OK" : "SMOKE FAILED: " + fails);
        return fails == 0 ? 0 : 1;
    }
}
