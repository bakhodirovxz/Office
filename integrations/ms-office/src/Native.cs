// P/Invoke of crates/uz-translit-ffi. The DLL is shipped twice (x86\ and x64\ next to this assembly); the one that
// matches the process bitness is loaded by full path before the first call, so DllImport binds to it.
using System;
using System.IO;
using System.Runtime.InteropServices;

namespace UzTranslit.Office
{
    static class Native
    {
        public const uint ToCyrillic = 0, ToLatin = 1;
        public const uint AlphabetAuto = 0, Alphabet2026 = 2;   // the FFI also takes 1 = 1995 (CLI/API); the add-in never passes it
        const uint AbiVersion = 1;
        const string Dll = "uz_translit_ffi";

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr LoadLibraryW(string path);

        [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern uint uzt_abi_version();
        [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
        static extern int uzt_convert([MarshalAs(UnmanagedType.LPWStr)] string text, uint len, uint to, uint alphabet, uint flags,
            out IntPtr result, out uint resultLen);
        [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
        static extern int uzt_convert_runs([MarshalAs(UnmanagedType.LPWStr)] string text, uint[] lens, byte[] convert, uint n,
            uint to, uint alphabet, uint flags, out IntPtr result, out uint resultLen, [Out] uint[] resultLens);
        [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern void uzt_free(IntPtr p, uint len);

        static readonly object gate = new object();
        static bool loaded;

        static void Load()
        {
            lock (gate)
            {
                if (loaded) return;
                string dir = Path.GetDirectoryName(typeof(Native).Assembly.Location);
                string path = Path.Combine(Path.Combine(dir, IntPtr.Size == 8 ? "x64" : "x86"), Dll + ".dll");
                if (LoadLibraryW(path) == IntPtr.Zero)
                    throw new DllNotFoundException(path + " (error " + Marshal.GetLastWin32Error() + ")");
                uint v = uzt_abi_version();
                if (v != AbiVersion) throw new InvalidOperationException("uz_translit_ffi ABI " + v + ", expected " + AbiVersion);
                loaded = true;
            }
        }

        static string Take(IntPtr p, uint len)
        {
            if (p == IntPtr.Zero) return "";
            try { return Marshal.PtrToStringUni(p, (int)len); } finally { uzt_free(p, len); }
        }

        static void Check(int rc)
        {
            if (rc != 0) throw new InvalidOperationException("uz_translit_ffi error " + rc);
        }

        public static string Convert(string text, uint to, uint alphabet)
        {
            Load();
            IntPtr p; uint n;
            Check(uzt_convert(text, (uint)text.Length, to, alphabet, 0, out p, out n));
            return Take(p, n);
        }

        /// The runs of one paragraph -> each run's new text (null = the engine refused this paragraph, e.g. bad UTF-16).
        public static string[] ConvertRuns(string[] runs, bool[] convert, uint to, uint alphabet)
        {
            Load();
            var lens = new uint[runs.Length];
            var conv = new byte[runs.Length];
            for (int i = 0; i < runs.Length; i++) { lens[i] = (uint)runs[i].Length; conv[i] = convert[i] ? (byte)1 : (byte)0; }
            var outLens = new uint[runs.Length];
            IntPtr p; uint n;
            int rc = uzt_convert_runs(string.Concat(runs), lens, conv, (uint)runs.Length, to, alphabet, 0, out p, out n, outLens);
            if (rc == 2) return null;
            Check(rc);
            string all = Take(p, n);
            var res = new string[runs.Length];
            int at = 0;
            for (int i = 0; i < runs.Length; i++) { res[i] = all.Substring(at, (int)outLens[i]); at += (int)outLens[i]; }
            return res;
        }
    }
}
