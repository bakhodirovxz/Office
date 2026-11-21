// UI strings from i18n/msaddin.tsv (embedded at build time), in the language of the Office UI.
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;

namespace UzTranslit.Office
{
    sealed class Catalog
    {
        public string Lang = "en";
        readonly Dictionary<string, string> strings = new Dictionary<string, string>();

        /// Office UI LCID -> catalog language. Uzbek (primary 0x43): sublanguage 2 is Cyrillic, anything else Latin;
        /// Office has one Uzbek Latin UI and no separate 2026 one, so uz-Latn strings are in the alphabet in force (1995).
        /// This is UI text only: the conversion always writes the 2026 alphabet. Russian = 0x19. Else en.
        public static string LangOf(int lcid)
        {
            int primary = lcid & 0x3FF, sub = lcid >> 10;
            if (primary == 0x43) return sub == 2 ? "uz_cyrl" : "uz_latn";
            if (primary == 0x19) return "ru";
            return "en";
        }

        public static Catalog Load(int lcid)
        {
            var c = new Catalog { Lang = LangOf(lcid) };
            using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("msaddin.tsv"))
            using (var r = new StreamReader(s))
            {
                string[] head = r.ReadLine().Split('\t');
                int ctx = Array.IndexOf(head, "context"), en = Array.IndexOf(head, "source_en");
                int col = c.Lang == "en" ? en : Array.IndexOf(head, c.Lang);
                string line;
                while ((line = r.ReadLine()) != null)
                {
                    var f = line.Split('\t');
                    if (f.Length <= Math.Max(col, ctx)) continue;
                    c.strings[f[ctx]] = f[col].Length > 0 ? f[col] : f[en];
                }
            }
            // the ribbon tab carries the product name (brand/brand.toml, via build.sh): line 1 = name, line 2 = its uz-Cyrl form (may be empty)
            using (var s = Assembly.GetExecutingAssembly().GetManifestResourceStream("brand.txt"))
            using (var r = new StreamReader(s))
            {
                string name = r.ReadLine() ?? "", cyr = r.ReadLine() ?? "";
                c.strings["ribbon.uztTab.label"] = c.Lang == "uz_cyrl" && cyr.Length > 0 ? cyr : name;
            }
            return c;
        }

        public string Get(string context)
        {
            string s;
            return strings.TryGetValue(context, out s) ? s : context;
        }
    }
}
