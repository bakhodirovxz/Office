// Smoke test of the Word add-in (docs/ms-addin/PLAN.md section 7). Starts its OWN Word instance through COM (a new
// /Automation process), never touches other Word windows, works only on files under <out>\test-docs, and quits its
// instance in `finally`. The add-in must be registered (run-smoke.ps1 registers it only for the length of the run).
//
//   smoke.exe <test-docs dir> <golden.tsv> [--big N]     exit 0 = every check passed
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;

static class Smoke
{
    static int fails;
    static dynamic addin;

    static void Check(bool ok, string what)
    {
        Console.WriteLine((ok ? "PASS " : "FAIL ") + what);
        if (!ok) fails++;
    }

    static string Vis(string s)
    {
        if (s == null) return "(null)";
        return string.Concat(s.Select(c => c < ' ' ? "<" + (int)c + ">" : c.ToString()));
    }

    /// Every story's text (default retrieval: field results, not codes), keyed by story type and chain index.
    static Dictionary<string, string> Stories(dynamic doc)
    {
        var d = new Dictionary<string, string>();
        foreach (dynamic s in doc.StoryRanges)
        {
            int k = 0;
            for (dynamic r = s; r != null; r = r.NextStoryRange) d[(int)s.StoryType + "." + k++] = (string)r.Text;
        }
        return d;
    }

    /// The formatting runs of a range (consecutive equal formats merged): name|size|b|i|u|color|highlight.
    static List<string> Runs(dynamic range)
    {
        var res = new List<string>();
        dynamic r = range.Duplicate;
        int end = range.End, pos = range.Start;
        while (pos < end)
        {
            r.SetRange(pos, pos);
            r.MoveEnd(13, 1);
            int e = Math.Min(Math.Max((int)r.End, pos + 1), end);
            r.SetRange(pos, e);
            dynamic f = r.Font;
            string sig = f.Name + "|" + f.Size + "|" + f.Bold + "|" + f.Italic + "|" + f.Underline + "|" + f.Color + "|" + r.HighlightColorIndex;
            if (res.Count == 0 || res[res.Count - 1] != sig) res.Add(sig);
            pos = e;
        }
        return res;
    }

    /// The run property sequence of the WordprocessingML (runs with text only, consecutive equal ones merged).
    static List<string> XmlRunProps(string xml)
    {
        var res = new List<string>();
        foreach (Match m in Regex.Matches(xml, @"<w:r(?:\s[^>]*)?>(.*?)</w:r>", RegexOptions.Singleline))
        {
            string body = m.Groups[1].Value;
            if (!body.Contains("<w:t")) continue;
            var p = Regex.Match(body, @"<w:rPr>.*?</w:rPr>", RegexOptions.Singleline);
            string sig = p.Success ? p.Value : "";
            if (res.Count == 0 || res[res.Count - 1] != sig) res.Add(sig);
        }
        return res;
    }

    static bool Same(List<string> a, List<string> b) { return a.SequenceEqual(b); }

    /// Equal, or print where the first difference is.
    static bool Diff(string got, string want)
    {
        if (got == want) return true;
        int i = 0;
        while (i < got.Length && i < want.Length && got[i] == want[i]) i++;
        int a = Math.Max(0, i - 30);
        Console.WriteLine("  first difference at " + i + " (len " + got.Length + " vs " + want.Length + "): got '" + Vis(got.Substring(a, Math.Min(70, got.Length - a))) + "' want '" + Vis(want.Substring(a, Math.Min(70, want.Length - a))) + "'");
        return false;
    }

    static List<string> FieldCodes(dynamic doc)
    {
        var l = new List<string>();
        foreach (dynamic s in doc.StoryRanges)
            for (dynamic r = s; r != null; r = r.NextStoryRange)
                foreach (dynamic f in r.Fields) l.Add((string)f.Code.Text);
        return l;
    }

    static string Expected(string text, string mode)
    {
        // Convert per paragraph, as the add-in does; control characters stay.
        var sb = new StringBuilder();
        foreach (var part in Regex.Split(text, "([\r\a\f\v\x01-\x08\x0e-\x1f])"))
            sb.Append(part.Length == 1 && part[0] < ' ' ? part : (string)addin.ConvertText(part, mode));
        return sb.ToString();
    }

    static dynamic NewWord()
    {
        return Activator.CreateInstance(Type.GetTypeFromProgID("Word.Application"));
    }

    static dynamic ConnectAddin(dynamic w)
    {
        dynamic ca = null;
        foreach (dynamic c in w.COMAddIns) if ((string)c.ProgId == "UzTranslit.OfficeAddin") ca = c;
        if (ca == null) throw new Exception("add-in not registered (COMAddIns has no UzTranslit.OfficeAddin)");
        bool loadedAtStart = ca.Connect;
        if (!loadedAtStart) ca.Connect = true;
        Console.WriteLine("add-in loaded at Word start: " + loadedAtStart + "; connected: " + ca.Connect);
        return ca.Object;
    }

    static dynamic BuildFeatureDoc(dynamic w, string path)
    {
        dynamic d = w.Documents.Add();
        dynamic c = d.Content;
        c.Text = "Shahar markazida yangi maktab qurildi.\rO'zbekiston Respublikasi mustaqil davlat. Choy, g'alla va shoir. \rPoytaxt Toshkent shahri.\rBugun havo issiq, bolalar bogʻda oʻynashdi va choy ichishdi.\rErtaga esa yomgʻir yogʻadi, shamol kuchayadi.\vYangi qator shu yerda.\rBirinchi band roʻyxatda.\rJadval:\r";
        Func<int, string, dynamic> word = (para, text) =>
        {
            dynamic pr = d.Paragraphs[para].Range;
            int at = ((string)pr.Text).IndexOf(text, StringComparison.Ordinal);
            return d.Range((int)pr.Start + at, (int)pr.Start + at + text.Length);
        };
        word(1, "Sha").Font.Bold = true;          // mid-word: Sha|har
        word(1, "har ").Font.Italic = true;
        word(1, "markazida").Font.Color = 255;    // red
        word(1, "yangi").Font.Underline = 1;
        dynamic mk = word(1, "maktab"); mk.Font.Name = "Arial"; mk.Font.Size = 14;
        word(1, "qurildi").HighlightColorIndex = 7; // yellow
        word(2, "mustaqil").Font.Bold = true;
        word(4, "Bu").Font.Bold = true;           // plain paragraphs 4-5: the WordXml block path
        word(4, "issiq").Font.Italic = true;
        word(4, "bolalar").Font.Color = 16711680;
        word(5, "yomgʻir").HighlightColorIndex = 4;
        word(5, "shamol").Font.Name = "Courier New";
        d.Paragraphs[6].Range.ListFormat.ApplyBulletDefault();  // a list item: never through InsertXML
        // Hyperlink (a field) at the end of paragraph 2, a content control around "davlat", a comment on "yangi".
        dynamic p2 = d.Paragraphs[2].Range;
        d.Hyperlinks.Add(d.Range((int)p2.End - 1, (int)p2.End - 1), "https://example.uz/sahifa", Type.Missing, Type.Missing, "havola matni");
        d.ContentControls.Add(1, word(2, "davlat"));     // wdContentControlRichText
        d.Comments.Add(word(1, "yangi"), "Sharh: yangi bino");
        d.Bookmarks.Add("bm1", word(3, "Toshkent"));
        d.Footnotes.Add(word(1, "qurildi"), Type.Missing, "Izoh: maktab ikki qavatli.");
        // Table.
        dynamic tp = d.Paragraphs[7].Range;
        dynamic t = d.Tables.Add(d.Range((int)tp.End, (int)tp.End), 2, 2);
        t.Cell(1, 1).Range.Text = "olma"; t.Cell(1, 2).Range.Text = "nok";
        t.Cell(2, 1).Range.Text = "uzum"; t.Cell(2, 2).Range.Text = "anor shirin";
        t.Cell(2, 2).Range.Words[1].Font.Bold = true;
        // Header with a text box, footer with a PAGE field.
        dynamic sec = d.Sections[1];
        sec.Headers[1].Range.Text = "Sarlavha: rasmiy hujjat";
        dynamic fr = sec.Footers[1].Range; fr.Text = "Sahifa ";
        dynamic fend = sec.Footers[1].Range; fend.Collapse(0); fend.MoveEnd(1, -1);
        d.Fields.Add(fend, 33);                   // wdFieldPage
        dynamic box = d.Shapes.AddTextbox(1, 300, 400, 150, 60);
        box.TextFrame.TextRange.Text = "Matn qutisi ichida";
        d.SaveAs2(FileName: path, FileFormat: 12, AddToRecentFiles: false);
        return d;
    }

    static void FeatureTests(dynamic w, string dir)
    {
        dynamic d = BuildFeatureDoc(w, Path.Combine(dir, "features.docx"));
        Dictionary<string, string> text0 = Stories(d);
        List<string> runs0 = Runs(d.Content);
        var xml0 = XmlRunProps(d.Content.WordOpenXML);
        List<string> codes0 = FieldCodes(d);
        int comments0 = d.Comments.Count, notes0 = d.Footnotes.Count, cc0 = d.ContentControls.Count, links0 = d.Hyperlinks.Count;
        Console.WriteLine("feature doc: " + text0.Count + " stories, " + runs0.Count + " formatting runs, " + codes0.Count + " fields");

        foreach (var mode in new[] { "cyrl", "latn", "cyrl", "latn" })
        {
            d.Range(0, 0).Select();               // nothing selected -> whole document
            Dictionary<string, string> before = Stories(d);
            string bm0 = d.Bookmarks["bm1"].Range.Text, cc0text = d.ContentControls[1].Range.Text, sc0 = d.Comments[1].Scope.Text;
            string stats = addin.Convert(mode, false);
            Check(stats.Contains("xml_inserts=0;"), mode + " the default is per run (one Undo step)");
            Console.WriteLine(mode + " whole document: " + stats);
            Dictionary<string, string> after = Stories(d);
            foreach (var k in before.Keys)
            {
                string want = Expected(before[k], mode);
                Check(after[k] == want, mode + " story " + k + " text" + (after[k] == want ? "" : ": got '" + Vis(after[k]) + "' want '" + Vis(want) + "'"));
            }
            Check(Same(Runs(d.Content), runs0), mode + " formatting runs (COM walk) unchanged");
            Check(Same(XmlRunProps(d.Content.WordOpenXML), xml0), mode + " run properties (WordOpenXML) unchanged");
            Check(Same(FieldCodes(d), codes0), mode + " field codes unchanged");
            Check(d.Comments.Count == comments0 && d.Footnotes.Count == notes0 && d.ContentControls.Count == cc0 && d.Hyperlinks.Count == links0,
                mode + " comments/footnotes/content controls/hyperlinks kept");
            Check((string)d.Hyperlinks[1].Address == "https://example.uz/sahifa", mode + " hyperlink address unchanged");
            string sc = d.Comments[1].Scope.Text;
            Check(sc == Expected(sc0, mode), mode + " comment still anchored on the converted word: '" + sc + "'");
            string bm = d.Bookmarks.Exists("bm1") ? (string)d.Bookmarks["bm1"].Range.Text : null;
            Check(bm == Expected(bm0, mode), mode + " bookmark covers the converted word: '" + bm + "'");
            string cc = d.ContentControls[1].Range.Text;
            Check(cc == Expected(cc0text, mode), mode + " content control text converted: '" + cc + "'");

            // One Undo restores everything.
            d.Undo(1);
            Dictionary<string, string> undone = Stories(d);
            string bad = before.Keys.FirstOrDefault(k => undone[k] != before[k]);
            Check(bad == null, mode + " one Undo restores the text of every story" + (bad == null ? "" : ": story " + bad + " is '" + Vis(undone[bad]) + "', was '" + Vis(before[bad]) + "'"));
            Check(Same(Runs(d.Content), runs0), mode + " one Undo restores the formatting runs");
            d.Redo(1);
            Dictionary<string, string> redone = Stories(d);
            Check(redone.Keys.All(k => redone[k] == after[k]), mode + " Redo re-applies");
        }

        // Selection: only the first paragraph changes (on the saved, unconverted file).
        string path = d.FullName;
        d.Close(false);
        d = w.Documents.Open(FileName: path, AddToRecentFiles: false);
        Dictionary<string, string> t0 = Stories(d);
        d.Paragraphs[1].Range.Select();
        Console.WriteLine("cyrl selection: " + (string)addin.Convert("cyrl", false));
        string p1 = d.Paragraphs[1].Range.Text, p2 = d.Paragraphs[2].Range.Text;
        Check(p1 == Expected("Shahar markazida yangi maktab qurildi.\u0002\r", "cyrl") || p1.StartsWith("Шаҳар марказида"), "selection: paragraph 1 converted: '" + Vis(p1) + "'");
        Check(p2.StartsWith("O'zbekiston"), "selection: paragraph 2 untouched");
        d.Undo(1);
        string u1 = Stories(d)["1.0"];
        Check(u1 == t0["1.0"], "selection: Undo restores" + (u1 == t0["1.0"] ? "" : ": '" + Vis(u1) + "' was '" + Vis(t0["1.0"]) + "'"));

        // Fast mode (WordXml, decision D8): the same checks except one-step Undo, which it cannot give.
        List<string> runsR = Runs(d.Content), xmlR = XmlRunProps(d.Content.WordOpenXML), codesR = FieldCodes(d);
        string fast = addin.ConvertFast("cyrl", true);
        Console.WriteLine("cyrl fast whole document: " + fast);
        Check(!fast.Contains("xml_inserts=0;"), "fast: the plain paragraphs went through WordXml");
        Dictionary<string, string> f1 = Stories(d);
        string fbad = t0.Keys.FirstOrDefault(k => f1[k] != Expected(t0[k], "cyrl"));
        Check(fbad == null, "fast: every story's text" + (fbad == null ? "" : ": story " + fbad + " '" + Vis(f1[fbad]) + "'"));
        Check(Same(Runs(d.Content), runsR), "fast: formatting runs (COM walk) unchanged");
        Check(Same(XmlRunProps(d.Content.WordOpenXML), xmlR), "fast: run properties (WordOpenXML) unchanged");
        Check(Same(FieldCodes(d), codesR), "fast: field codes unchanged");
        int fsteps = 0;
        while (fsteps < 100)
        {
            Dictionary<string, string> cur = Stories(d);
            string v;
            if (cur.All(kv => t0.TryGetValue(kv.Key, out v) && v == kv.Value)) break;
            d.Undo(1);
            fsteps++;
        }
        Console.WriteLine("fast: Undo steps to restore the feature document: " + fsteps);

        // Refusals.
        d.TrackRevisions = true;
        d.Range(0, 0).InsertBefore("X");
        d.TrackRevisions = false;
        try { addin.Convert("cyrl", true); Check(false, "tracked changes are refused"); }
        catch (Exception e) { Check(e.Message.Contains("msg.tracked") || (e.InnerException != null && e.InnerException.Message.Contains("msg.tracked")) || e.ToString().Contains("msg.tracked"), "tracked changes are refused (" + e.GetType().Name + ")"); }
        d.Revisions.RejectAll();
        d.Save();
        d.Close(false);
    }

    /// A long document: paragraphs of golden-list words with a formatting change every few words, some mid-word.
    static string BigHtml(string golden, int paragraphs)
    {
        var words = File.ReadAllLines(golden).Skip(1).Select(l => l.Split('\t')).Where(f => f.Length > 3 && f[5].StartsWith("reviewed:"))
            .Select(f => f[1]).Where(x => x.Length > 2 && !x.Contains('<') && !x.Contains('&')).ToArray();
        var rnd = new Random(42);
        var sb = new StringBuilder("<html><head><meta charset=\"utf-8\"></head><body>");
        for (int p = 0; p < paragraphs; p++)
        {
            sb.Append("<p>");
            for (int i = 0; i < 60; i++)
            {
                string x = words[rnd.Next(words.Length)];
                int k = rnd.Next(10);
                if (k == 0) sb.Append("<b>" + x + "</b>");
                else if (k == 1) sb.Append("<i>" + x + "</i>");
                else if (k == 2) sb.Append("<span style=\"color:#c00000\">" + x + "</span>");
                else if (k == 3 && x.Length > 3) sb.Append("<b>" + x.Substring(0, 2) + "</b>" + x.Substring(2));
                else sb.Append(x);
                sb.Append(i % 12 == 11 ? ". " : " ");
            }
            sb.Append("</p>");
        }
        return sb.Append("</body></html>").ToString();
    }

    static void BigTest(dynamic w, string dir, string golden, int paragraphs, bool fastOnly)
    {
        string html = Path.Combine(dir, "big.htm"), docx = Path.Combine(dir, "big.docx");
        File.WriteAllText(html, BigHtml(golden, paragraphs), new UTF8Encoding(false));
        dynamic d = w.Documents.Open(FileName: html, ConfirmConversions: false, AddToRecentFiles: false);
        d.SaveAs2(FileName: docx, FileFormat: 12, AddToRecentFiles: false);
        d.ActiveWindow.View.Type = 3;          // print layout: an opened .htm stays in web layout, which has one "page"
        int pages = d.ComputeStatistics(2), wordsN = d.ComputeStatistics(0), chars = d.ComputeStatistics(3);
        string text0 = d.Content.Text;
        var xml0 = XmlRunProps(d.Content.WordOpenXML);
        Console.WriteLine("big doc: " + pages + " pages, " + wordsN + " words, " + chars + " characters, " + xml0.Count + " formatting runs");

        d.Range(0, 0).Select();
        var sw = Stopwatch.StartNew();
        string text1 = Expected(text0, "cyrl");
        if (!fastOnly)
        {
            string s1 = addin.Convert("cyrl", false);
            long t1 = sw.ElapsedMilliseconds;
            Console.WriteLine("big cyrl: " + s1 + " (wall " + t1 + " ms)");
            Check(Diff(d.Content.Text, text1), "big: text equals the engine's conversion");
            Check(Same(XmlRunProps(d.Content.WordOpenXML), xml0), "big: run properties unchanged (" + xml0.Count + " runs)");
            sw.Restart();
            d.Undo(1);
            long tu = sw.ElapsedMilliseconds;
            Check((string)d.Content.Text == text0, "big: one Undo restores (" + tu + " ms)");
        }

        // Fast mode (WordXml, decision D8): same result, but how many Undo steps?
        sw.Restart();
        string f1 = addin.ConvertFast("cyrl", false);
        Console.WriteLine("big cyrl fast: " + f1 + " (wall " + sw.ElapsedMilliseconds + " ms)");
        Check(Diff(d.Content.Text, text1), "big fast: text equals the engine's conversion");
        Check(Same(XmlRunProps(d.Content.WordOpenXML), xml0), "big fast: run properties unchanged (" + xml0.Count + " runs)");
        sw.Restart();
        int steps = 0;
        while ((string)d.Content.Text != text0 && steps < 500) { d.Undo(1); steps++; }
        Console.WriteLine("big fast: Undo steps to restore: " + steps + " (" + sw.ElapsedMilliseconds + " ms)");
        addin.ConvertFast("cyrl", false);
        sw.Restart();
        string f2 = addin.ConvertFast("latn", false);
        Console.WriteLine("big latn fast (from the Cyrillic): " + f2 + " (wall " + sw.ElapsedMilliseconds + " ms)");
        Check(Diff(d.Content.Text, Expected(text1, "latn")), "big fast: back to 2026 Latin equals the engine's conversion");
        Check(Same(XmlRunProps(d.Content.WordOpenXML), xml0), "big fast: run properties unchanged after the second conversion");
        d.Close(false);
    }

    static int Main(string[] args)
    {
        string dir = args[0], golden = args[1];
        int big = args.Length > 3 && args[2] == "--big" ? int.Parse(args[3]) : 0;
        Directory.CreateDirectory(dir);
        dynamic w = NewWord();
        try
        {
            Console.WriteLine("Word " + w.Version + " build " + w.Build + ", UI LCID " + w.LanguageSettings.LanguageID[2]);
            w.DisplayAlerts = 0;
            addin = ConnectAddin(w);
            Console.WriteLine("add-in UI language: " + addin.UiLanguage());
            foreach (int lcid in new[] { 1033, 1049, 1091, 2115, 1031 })
                Console.WriteLine("  label uztToLatn @" + lcid + ": " + addin.CatalogGet("ribbon.uztToLatn.label", lcid));
            Check((string)addin.CatalogGet("ribbon.uztToCyrl.label", 2115) == "Лотинчадан кириллчага", "catalog: uz-Cyrl UI picks the uz-Cyrl string");
            Check((string)addin.CatalogGet("ribbon.uztToCyrl.label", 1031) == "Latin to Cyrillic", "catalog: an unknown UI language falls back to en");
            w.WindowState = 2; w.Visible = true;   // the ribbon and GetImageMso need a UI; our own instance, minimized, for these probes only
            for (int i = 0; i < 20 && !((string)addin.RibbonState()).Contains("labels=") ; i++) System.Threading.Thread.Sleep(250);
            System.Threading.Thread.Sleep(2000);
            Console.WriteLine("ribbon: " + addin.RibbonState());
            Check(((string)addin.RibbonState()).StartsWith("loaded=True"), "ribbon XML loaded (onLoad called)");
            // onLoad fires even when the tab id is wrong; labels are asked only for a group the ribbon really shows (Word's Review
            // tab is TabReviewWord, not TabReview). The window stays minimized: a shown one repaginates the test documents.
            addin.ShowReviewTab(); System.Threading.Thread.Sleep(1500);
            Console.WriteLine("ribbon on the Review tab: " + addin.RibbonState());
            Check(!((string)addin.RibbonState()).EndsWith("labels=0"), "the group is on Word's Review tab (getLabel called)");
            string icons = addin.IconProbe();
            Console.WriteLine("icons (getImage): " + icons);
            Check(!icons.Contains("null") && icons.Split(';').Length == 2, "getImage returns a picture for each button");
            Console.WriteLine("imageMso present: " + addin.ProbeImageMso("ChangeCase,ChangeCaseGallery,Translate,TranslateMenu,TranslationPane,SetLanguage,LanguageSetting,TextDirection,TextDirectionMenu,FontDialog,Font,Bold,WordArtInsertGallery,ReviewTranslate,ProofingMenu,SpellingAndGrammar,ResearchPane,ThesaurusPane,TranslateDocument,TranslateSelection,ShowClipboard,ReplaceDialog,EncodingMenu,ChineseTranslationDialog,KoreanHanjaConvert,HangulHanjaConversion,TransliterationPane,ConvertText,TextBoxInsert"));
            w.Visible = false;
            FeatureTests(w, dir);
            if (big > 0) BigTest(w, dir, golden, big, args.Contains("--fast-only"));
        }
        catch (Exception e)
        {
            Console.WriteLine("FAIL exception: " + e);
            fails++;
        }
        finally
        {
            try { foreach (dynamic d in w.Documents) d.Close(false); } catch { }
            try { w.Quit(false); } catch { }
        }
        Console.WriteLine(fails == 0 ? "SMOKE OK" : "SMOKE FAILED: " + fails);
        return fails == 0 ? 0 : 1;
    }
}
