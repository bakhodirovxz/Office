"""segheap_patch.py IN.exe OUT.exe -> a copy of IN whose embedded application manifest (MinGW's
default one) also opts into the Windows segment heap (`heapType SegmentHeap`, Windows 10 2004+).

For A/B measurements only: the manifest resource is rewritten in place at the same length (the
default manifest's comments make room; the rest is padded with spaces), so nothing else in the
binary changes. The product change is the build-time manifest (apps/*/build.rs).
"""
import re
import sys

src, dst = sys.argv[1:3]
d = bytearray(open(src, "rb").read())
start = d.find(b"<assembly")
end = d.find(b"</assembly>", start) + len(b"</assembly>")
assert start > 0 and end > start and d.find(b"<assembly", end) < 0, "expected exactly one embedded manifest"
old = d[start:end].decode("ascii")
assert "heapType" not in old
new = re.sub(r"<!--.*?-->", "", old)
new = re.sub(r"\n\s*\n", "\n", new)
new = new.replace(
    "</assembly>",
    '  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>'
    '<heapType xmlns="http://schemas.microsoft.com/SMI/2020/WindowsSettings">SegmentHeap</heapType>'
    "</windowsSettings></application>\n</assembly>",
)
assert len(new) <= len(old), (len(new), len(old))
d[start:end] = (new + " " * (len(old) - len(new))).encode("ascii")
open(dst, "wb").write(d)
print(f"patched manifest at {start:#x}: {len(old)} bytes")
