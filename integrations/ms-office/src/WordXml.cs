// Fast path for plain paragraphs (docs/ms-addin/PLAN.md section 4.1): Range.WordOpenXML -> convert the w:t text of
// each paragraph as runs -> Range.InsertXML. One read and one write per group of paragraphs instead of several Word
// calls per formatting run. Only paragraphs inside a strict allow-list qualify; everything else (fields, bookmarks,
// comments, content controls, notes, objects, lists, section breaks, hidden text, tables) takes the per-run path.
using System.Collections.Generic;
using System.Text;
using System.Xml;

namespace UzTranslit.Office
{
    static class WordXml
    {
        const string W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
        const string Pkg = "http://schemas.microsoft.com/office/2006/xmlPackage";

        public static XmlDocument Load(string package)
        {
            var doc = new XmlDocument { PreserveWhitespace = true };
            doc.LoadXml(package);
            return doc;
        }

        static XmlNode Body(XmlDocument doc)
        {
            var ns = new XmlNamespaceManager(doc.NameTable);
            ns.AddNamespace("pkg", Pkg);
            ns.AddNamespace("w", W);
            return doc.SelectSingleNode("/pkg:package/pkg:part[@pkg:name='/word/document.xml']/pkg:xmlData/w:document/w:body", ns);
        }

        /// The body paragraphs, or null if the body holds anything but paragraphs (a table, a block content control...).
        static List<XmlElement> Paragraphs(XmlDocument doc)
        {
            var body = Body(doc);
            if (body == null) return null;
            var ps = new List<XmlElement>();
            foreach (XmlNode c in body.ChildNodes)
            {
                if (c.NodeType != XmlNodeType.Element) continue;
                if (c.NamespaceURI == W && c.LocalName == "sectPr") continue;   // the package's own section properties
                if (c.NamespaceURI != W || c.LocalName != "p") return null;
                ps.Add((XmlElement)c);
            }
            return ps;
        }

        /// Per paragraph: plain enough for InsertXML? Null when the package does not hold exactly `n` paragraphs.
        public static bool[] Classify(XmlDocument doc, int n)
        {
            var ps = Paragraphs(doc);
            if (ps == null || ps.Count != n) return null;
            var res = new bool[n];
            for (int i = 0; i < n; i++) res[i] = Collect(ps[i], null, null, null);
            return res;
        }

        /// `doc` (read from a range that ends just before a paragraph mark) converted, and the text Word must show
        /// afterwards (inner paragraph marks included, not the final one), and the text the package held before
        /// (`source`: the caller compares it with Range.Text, so text the export lost is never written back).
        /// Null if a paragraph does not qualify.
        /// The final mark stays out of the range on purpose: InsertXML of a range with its last mark merged that
        /// paragraph into the next one in a probe (new document), and the existing mark keeps the paragraph properties.
        public static string Convert(XmlDocument doc, int n, Mode mode, out string text, out string source)
        {
            text = null;
            source = null;
            var ps = Paragraphs(doc);
            if (ps == null || ps.Count != n) return null;
            var sb = new StringBuilder();
            var src = new StringBuilder();
            for (int i = 0; i < n; i++)
            {
                var texts = new List<string>();
                var conv = new List<bool>();
                var nodes = new List<XmlElement>();
                if (!Collect(ps[i], texts, conv, nodes)) return null;
                if (texts.Count > 0)
                {
                    string[] res = Native.ConvertRuns(texts.ToArray(), conv.ToArray(), mode.To, mode.Alphabet, mode.Flags);
                    if (res == null) return null;
                    for (int k = 0; k < res.Length; k++)
                    {
                        if (nodes[k] == null) { if (res[k] != texts[k]) return null; }
                        else if (res[k] != texts[k])
                        {
                            nodes[k].InnerText = res[k];
                            // xml:space with the reserved "xml" prefix (SetAttribute(name, ns) without a prefix
                            // declares a generated one for it, which is not well-formed XML and InsertXML refuses).
                            var space = doc.CreateAttribute("xml", "space", "http://www.w3.org/XML/1998/namespace");
                            space.Value = "preserve";
                            nodes[k].Attributes.SetNamedItem(space);
                        }
                        sb.Append(res[k]);
                        src.Append(texts[k]);
                    }
                }
                if (i < n - 1) { sb.Append('\r'); src.Append('\r'); }
            }
            text = sb.ToString();
            source = src.ToString();
            return doc.OuterXml;
        }

        /// Walks one paragraph against the allow-list; fills the run lists when they are given.
        /// w:t = text, w:tab = "\t" (text, as in Range.Text), w:br/w:cr = "\v" (a hard boundary, as on the per-run path).
        static bool Collect(XmlElement p, List<string> texts, List<bool> conv, List<XmlElement> nodes)
        {
            foreach (XmlNode c in p.ChildNodes)
            {
                if (c.NodeType != XmlNodeType.Element) continue;
                if (c.NamespaceURI != W) return false;
                switch (c.LocalName)
                {
                    case "pPr":
                        // Lists and section breaks: InsertXML may renumber or re-create them.
                        var e = (XmlElement)c;
                        if (e.GetElementsByTagName("numPr", W).Count > 0 || e.GetElementsByTagName("sectPr", W).Count > 0) return false;
                        break;
                    case "proofErr":
                        break;
                    case "r":
                        foreach (XmlNode x in c.ChildNodes)
                        {
                            if (x.NodeType != XmlNodeType.Element) continue;
                            if (x.NamespaceURI != W) return false;
                            switch (x.LocalName)
                            {
                                case "rPr":
                                    // Hidden text: Range.Text may not show it, so the read-back check could not see it.
                                    if (((XmlElement)x).GetElementsByTagName("vanish", W).Count > 0) return false;
                                    break;
                                case "lastRenderedPageBreak": break;
                                case "t": Add(texts, conv, nodes, x.InnerText, true, (XmlElement)x); break;
                                case "tab": Add(texts, conv, nodes, "\t", true, null); break;
                                case "br": case "cr": Add(texts, conv, nodes, "\v", false, null); break;
                                default: return false;
                            }
                        }
                        break;
                    default:
                        return false;
                }
            }
            return true;
        }

        static void Add(List<string> texts, List<bool> conv, List<XmlElement> nodes, string t, bool c, XmlElement n)
        {
            if (texts == null) return;
            texts.Add(t); conv.Add(c); nodes.Add(n);
        }
    }
}
