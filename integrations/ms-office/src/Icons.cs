// Ribbon icons for getImage (customUI.xml): the word "Uzbek" in the alphabet each button writes - "Ўз" (Cyrillic),
// "Öz" (Latin, 2026 alphabet). Drawn at 32 px times the screen DPI scale, so they stay sharp on high-DPI
// screens (Office scales a larger image down, never a smaller one up cleanly). Returned as IPictureDisp, the type
// getImage expects in Office 2010+.
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Text;
using System.Windows.Forms;

namespace UzTranslit.Office
{
    static class Icons
    {
        public static int Size()
        {
            using (var g = Graphics.FromHwnd(IntPtr.Zero)) return (int)Math.Round(32 * g.DpiX / 96f);
        }

        public static Bitmap Draw(string tag, int px)
        {
            string text = tag == "cyrl" ? "Ўз" : "Öz";
            var bmp = new Bitmap(px, px, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(bmp))
            {
                g.Clear(Color.Transparent);
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
                // Office-like: dark grey glyphs, an accent bar in the Word blue.
                using (var font = new Font("Segoe UI Semibold", px * 0.44f, FontStyle.Regular, GraphicsUnit.Pixel))
                using (var ink = new SolidBrush(Color.FromArgb(255, 0x44, 0x44, 0x44)))
                using (var accent = new SolidBrush(Color.FromArgb(255, 0x2B, 0x57, 0x9A)))
                using (var fmt = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
                {
                    g.DrawString(text, font, ink, new RectangleF(0, -px * 0.06f, px, px * 0.86f), fmt);
                    g.FillRectangle(accent, px * 0.12f, px * 0.80f, px * 0.76f, Math.Max(2f, px * 0.09f));
                }
            }
            return bmp;
        }

        /// IPictureDisp of the icon (AxHost's protected converter, the usual route for shared COM add-ins).
        public static object Picture(string tag) { return Converter.Convert(Draw(tag, Size())); }

        sealed class Converter : AxHost
        {
            Converter() : base("") { }
            public static object Convert(Image image) { return GetIPictureDispFromPicture(image); }
        }
    }
}
