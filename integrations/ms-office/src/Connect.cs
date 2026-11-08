// The shared COM add-in (docs/ms-addin/PLAN.md section 3): one class, registered once, loaded by Word, Excel and
// PowerPoint. Office talks to it through IDTExtensibility2 (load/unload) and IRibbonExtensibility (the ribbon XML);
// ribbon callbacks and the test API arrive by name through IDispatch on IAddin. No Office PIA is referenced: the two
// Office interfaces are declared below from their public IIDs, and the object models are used late-bound (dynamic).
using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;

namespace UzTranslit.Office
{
    [ComImport, Guid("B65AD801-ABAF-11D0-BB8B-00A0C90F2744"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
    public interface IDTExtensibility2
    {
        [DispId(1)] void OnConnection([MarshalAs(UnmanagedType.IDispatch)] object application, int connectMode,
            [MarshalAs(UnmanagedType.IDispatch)] object addInInst, [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_VARIANT)] ref Array custom);
        [DispId(2)] void OnDisconnection(int removeMode, [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_VARIANT)] ref Array custom);
        [DispId(3)] void OnAddInsUpdate([MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_VARIANT)] ref Array custom);
        [DispId(4)] void OnStartupComplete([MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_VARIANT)] ref Array custom);
        [DispId(5)] void OnBeginShutdown([MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_VARIANT)] ref Array custom);
    }

    [ComImport, Guid("000C0396-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
    public interface IRibbonExtensibility
    {
        [DispId(1)] [return: MarshalAs(UnmanagedType.BStr)] string GetCustomUI([MarshalAs(UnmanagedType.BStr)] string ribbonId);
    }

    /// The default IDispatch of the add-in: ribbon callbacks (customUI.xml) and the API the smoke test calls through
    /// COMAddIn.Object. Listed first on Connect so that it is the default interface.
    [ComVisible(true), Guid("6E0B6A8C-3C0B-4F5E-9C59-0C3E4C1D2A71"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
    public interface IAddin
    {
        void OnRibbonLoad([MarshalAs(UnmanagedType.IDispatch)] object ribbon);
        string GetLabel([MarshalAs(UnmanagedType.IDispatch)] object control);
        string GetScreentip([MarshalAs(UnmanagedType.IDispatch)] object control);
        string GetSupertip([MarshalAs(UnmanagedType.IDispatch)] object control);
        object GetImage([MarshalAs(UnmanagedType.IDispatch)] object control);
        void OnConvert([MarshalAs(UnmanagedType.IDispatch)] object control);
        /// mode: "cyrl" (reads 1995 or 2026 Latin per paragraph) or "latn" (writes the 2026 alphabet). Returns "paragraphs=..;edits=..;skipped=..;ms=..", throws on failure.
        string Convert(string mode, bool wholeDocument);
        /// Convert with the WordXml fast path (on the ribbon only after the large-document question: decision D8).
        string ConvertFast(string mode, bool wholeDocument);
        /// Excel: scope "selection" | "sheet" | "workbook"; PowerPoint: "selection" | "presentation" | "presentation+masters".
        string ConvertIn(string mode, string scope);
        /// Undo transliteration for the active workbook/presentation: "restored=..;skipped=.." or "nothing".
        string UndoLast();
        bool GetVisible([MarshalAs(UnmanagedType.IDispatch)] object control);
        void OnUndo([MarshalAs(UnmanagedType.IDispatch)] object control);
        string ConvertText(string text, string mode);
        string UiLanguage();
        string ProbeImageMso(string names);
        /// A catalog string for an Office UI LCID (what the ribbon would show under that UI language).
        string CatalogGet(string context, int lcid);
        /// "loaded=<onLoad called>;labels=<getLabel calls>" (the smoke test checks the ribbon XML really loads).
        string RibbonState();
        /// Shows the Review tab (IRibbonUI.ActivateTabMso), so the ribbon asks for the group's labels if the group is there.
        string ShowReviewTab();
        /// Per button: the icon getImage returns, as "tag:WxH px" (IPictureDisp size), for the smoke test.
        string IconProbe();
    }

    [ComVisible(true), Guid("B1F8F147-98CA-4DF8-8C5D-E2E2D4E4E3A8"), ProgId("UzTranslit.OfficeAddin"),
     ClassInterface(ClassInterfaceType.None), ComDefaultInterface(typeof(IAddin))]
    public sealed class Connect : IAddin, IDTExtensibility2, IRibbonExtensibility
    {
        dynamic app;
        Host host = Host.Other;
        Catalog cat = Catalog.Load(1033);

        enum Host { Word, Excel, PowerPoint, Other }

        public void OnConnection(object application, int connectMode, object addInInst, ref Array custom)
        {
            app = application;
            string name = "";
            try { name = app.Name; } catch { }
            host = name.Contains("Word") ? Host.Word : name.Contains("Excel") ? Host.Excel
                 : name.Contains("PowerPoint") ? Host.PowerPoint : Host.Other;
            cat = Catalog.Load(UiLcid());
            try { ((dynamic)addInInst).Object = this; } catch { }
        }

        public void OnDisconnection(int removeMode, ref Array custom) { Release(); }
        public void OnAddInsUpdate(ref Array custom) { }
        public void OnStartupComplete(ref Array custom) { }
        public void OnBeginShutdown(ref Array custom) { Release(); }

        /// Undo snapshots hold references to cells and shapes; while any reference to the host is alive, Excel does not
        /// exit after Quit (measured: a hidden Excel stayed). Drop them and let the CLR release the RCWs now.
        void Release()
        {
            snapshots.Clear();
            app = null;
            GC.Collect(); GC.WaitForPendingFinalizers();
            GC.Collect(); GC.WaitForPendingFinalizers();
        }

        /// msoLanguageIDUI = 2. Office 2010+ has LanguageSettings on every app.
        int UiLcid()
        {
            try { return app.LanguageSettings.LanguageID[2]; } catch { return 1033; }
        }

        public string GetCustomUI(string ribbonId)
        {
            // One ribbon for the main window of every host (Outlook and other ribbons: nothing).
            if (ribbonId != "Microsoft.Word.Document" && ribbonId != "Microsoft.Excel.Workbook" && ribbonId != "Microsoft.PowerPoint.Presentation")
                return null;
            using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("customUI.xml"))
            using (var r = new StreamReader(s))
            {
                string xml = r.ReadToEnd();
                // Word's Review tab is TabReviewWord; TabReview there fails ("Failed to find Office control by ID") and the group is not shown.
                return host == Host.Word ? xml.Replace("idMso=\"TabReview\"", "idMso=\"TabReviewWord\"") : xml;
            }
        }

        /// "Undo transliteration" only where the host's own Undo cannot reverse the command (Excel, PowerPoint).
        public bool GetVisible(object control) { return !Id(control).StartsWith("uztUndo") || host != Host.Word; }

        bool ribbonLoaded;
        int labelCalls;
        object ribbonUi;
        public void OnRibbonLoad(object ribbon) { ribbonLoaded = true; ribbonUi = ribbon; }
        public string ShowReviewTab() { ((dynamic)ribbonUi).ActivateTabMso(host == Host.Word ? "TabReviewWord" : "TabReview"); return RibbonState(); }
        public string RibbonState() { return "loaded=" + ribbonLoaded + ";labels=" + labelCalls; }

        static string Id(object control) { return ((dynamic)control).Id; }
        static string Tag(object control) { return ((dynamic)control).Tag; }

        public string GetLabel(object control) { labelCalls++; return cat.Get("ribbon." + Id(control) + ".label"); }
        public string GetScreentip(object control) { return cat.Get("ribbon." + Id(control) + ".label"); }
        public string GetSupertip(object control) { return cat.Get("ribbon." + Id(control) + ".supertip"); }
        public object GetImage(object control) { return Icons.Picture(Tag(control)); }

        public string IconProbe()
        {
            var parts = new System.Collections.Generic.List<string>();
            foreach (var t in new[] { "cyrl", "latn" })
            {
                dynamic p = Icons.Picture(t);
                // IPictureDisp Width/Height are HIMETRIC (0.01 mm); 96 px = 2540 HIMETRIC at 100 %.
                parts.Add(t + ":" + Icons.Size() + "px:" + (p == null ? "null" : (int)p.Width + "x" + (int)p.Height + "himetric"));
            }
            return string.Join(";", parts.ToArray());
        }

        /// A Word document longer than this (characters) asks before converting whether to use the fast mode (D8).
        const int LargeDocument = 300000;
        /// The last conversion per workbook/presentation (Excel, PowerPoint), for Undo transliteration.
        readonly System.Collections.Generic.Dictionary<string, Snapshot> snapshots = new System.Collections.Generic.Dictionary<string, Snapshot>();

        string Caption { get { return cat.Get("ribbon.uztGroup.label"); } }

        public void OnConvert(object control)
        {
            try
            {
                string mode = Tag(control);
                Stats st;
                if (host == Host.Word)
                {
                    bool fast = false;
                    if ((int)app.ActiveDocument.Content.End > LargeDocument && (int)app.Selection.Start == (int)app.Selection.End)
                    {
                        int a = Ui.Ask(Caption, cat.Get("msg.fast_prompt"));
                        if (a == Ui.Cancel) return;
                        fast = a == Ui.Yes;
                    }
                    st = WordAdapter.Convert(app, Mode.Parse(mode), false, Caption, fast);
                }
                else
                {
                    string scope = "selection";
                    if (host == Host.Excel)
                    {
                        bool oneCell = false;
                        try { oneCell = (long)app.Selection.CountLarge == 1; } catch { }
                        if (oneCell)
                        {
                            int a = Ui.Ask(Caption, cat.Get("msg.excel_scope"));
                            if (a == Ui.Cancel) return;
                            scope = a == Ui.Yes ? "workbook" : "sheet";
                        }
                    }
                    st = Other(mode, scope);
                }
                string msg = st.Edits == 0 ? cat.Get("msg.nothing")
                    : host == Host.Word ? cat.Get("msg.done").Replace("{n}", st.Paragraphs.ToString())
                    : cat.Get("msg.done_items").Replace("{n}", st.Edits.ToString());
                if (st.Skipped > 0) msg += " " + cat.Get("msg.skipped").Replace("{n}", st.Skipped.ToString());
                if (st.Formulas > 0) msg += " " + cat.Get("msg.formulas").Replace("{n}", st.Formulas.ToString());
                if (host != Host.Word && st.Edits > 0) msg += " " + cat.Get("msg.own_undo");
                Ui.Done(app, host == Host.PowerPoint, Caption, msg);
            }
            catch (UserMessage e) { Ui.Warn(Caption, cat.Get(e.Key)); }
            catch (Exception e) { Ui.Warn(Caption, cat.Get("msg.failed").Replace("{text}", e.Message)); }
        }

        public void OnUndo(object control)
        {
            try
            {
                int[] r = UndoNow();
                string msg = r == null ? cat.Get("msg.undo_nothing") : cat.Get("msg.undo_done").Replace("{n}", r[0].ToString());
                if (r != null && r[1] > 0) msg += " " + cat.Get("msg.undo_skipped").Replace("{n}", r[1].ToString());
                Ui.Done(app, host == Host.PowerPoint, Caption, msg);
            }
            catch (Exception e) { Ui.Warn(Caption, cat.Get("msg.failed").Replace("{text}", e.Message)); }
        }

        string DocName()
        {
            return host == Host.Excel ? (string)app.ActiveWorkbook.FullName : (string)PowerPointAdapter.Presentation(app).FullName;
        }

        /// Excel or PowerPoint; a failure part-way puts back what was already changed.
        Stats Other(string mode, string scope)
        {
            var snap = new Snapshot();
            Stats st;
            try
            {
                st = host == Host.Excel ? ExcelAdapter.Convert(app, Mode.Parse(mode), scope, snap)
                   : host == Host.PowerPoint ? PowerPointAdapter.Convert(app, Mode.Parse(mode), scope, snap)
                   : null;
                if (st == null) throw new InvalidOperationException("unsupported host");
            }
            catch
            {
                Restore(snap);
                throw;
            }
            if (snap.Count > 0) snapshots[snap.Doc] = snap;
            return st;
        }

        int[] Restore(Snapshot snap)
        {
            return host == Host.Excel ? ExcelAdapter.Restore(app, snap) : PowerPointAdapter.Restore(snap);
        }

        /// (restored, skipped), or null if this document has nothing to undo.
        int[] UndoNow()
        {
            Snapshot snap;
            string doc = DocName();
            if (!snapshots.TryGetValue(doc, out snap)) return null;
            snapshots.Remove(doc);
            return Restore(snap);
        }

        public string Convert(string mode, bool wholeDocument) { return Api(() => WordAdapter.Convert(app, Mode.Parse(mode), wholeDocument, Caption, false)); }
        public string ConvertFast(string mode, bool wholeDocument) { return Api(() => WordAdapter.Convert(app, Mode.Parse(mode), wholeDocument, Caption, true)); }
        public string ConvertIn(string mode, string scope) { return Api(() => Other(mode, scope)); }

        public string UndoLast()
        {
            int[] r = UndoNow();
            return r == null ? "nothing" : "restored=" + r[0] + ";skipped=" + r[1];
        }

        static string Api(Func<Stats> run)
        {
            try { return run().ToString(); }
            catch (UserMessage e) { throw new InvalidOperationException(e.Key); }
        }

        public string ConvertText(string text, string mode) { var m = Mode.Parse(mode); return Native.Convert(text, m.To, m.Alphabet); }
        public string UiLanguage() { return cat.Lang; }
        public string CatalogGet(string context, int lcid) { return Catalog.Load(lcid).Get(context); }

        /// Which built-in icon names exist in this Office build (CommandBars.GetImageMso, in-process).
        public string ProbeImageMso(string names)
        {
            var ok = new System.Collections.Generic.List<string>();
            string err = null;
            foreach (var n in names.Split(','))
                try { object p = app.CommandBars.GetImageMso(n, 32, 32); if (p != null) ok.Add(n); }
                catch (Exception e) { if (err == null) err = n + ": " + e.Message; }
            return string.Join(",", ok.ToArray()) + (err == null ? "" : " | first error " + err);
        }
    }

    /// A refusal the user can act on; Key is a catalog context.
    sealed class UserMessage : Exception
    {
        public readonly string Key;
        public UserMessage(string key) : base(key) { Key = key; }
    }

    struct Mode
    {
        public uint To, Alphabet;
        public static Mode Parse(string s)
        {
            switch (s)
            {
                case "cyrl": return new Mode { To = Native.ToCyrillic, Alphabet = Native.AlphabetAuto };
                // Decision 2026-10-29: Latin is always written in the 2026 alphabet; 1995 Latin is read, never written.
                case "latn": return new Mode { To = Native.ToLatin, Alphabet = Native.Alphabet2026 };
                default: throw new ArgumentException("mode: " + s);
            }
        }
    }

    static class Ui
    {
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int MessageBoxW(IntPtr hwnd, string text, string caption, uint type);
        [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
        public const int Yes = 6, No = 7, Cancel = 2;
        /// The standard Windows message box (MB_ICONWARNING), owned by the Office window in front.
        public static void Warn(string caption, string text) { MessageBoxW(GetForegroundWindow(), text, caption, 0x30); }
        /// Yes / No / Cancel with the question icon (MB_YESNOCANCEL | MB_ICONQUESTION).
        public static int Ask(string caption, string text) { return MessageBoxW(GetForegroundWindow(), text, caption, 0x23); }

        /// Success: Word and Excel status bar (Excel's stays until reset, so it is cleared after 8 s); PowerPoint has
        /// no status bar in its object model, so an information box (MB_ICONINFORMATION).
        public static void Done(dynamic app, bool box, string caption, string text)
        {
            if (box) { MessageBoxW(GetForegroundWindow(), text, caption, 0x40); return; }
            try { app.StatusBar = text; } catch { return; }
            if (!((string)app.Name).Contains("Excel")) return;
            var t = new System.Windows.Forms.Timer { Interval = 8000 };
            t.Tick += (s, e) => { t.Stop(); t.Dispose(); try { app.StatusBar = false; } catch { } };
            t.Start();
        }
    }
}
