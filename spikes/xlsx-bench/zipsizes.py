import sys, zipfile
for p in sys.argv[1:]:
    z = zipfile.ZipFile(p)
    tot = sum(i.file_size for i in z.infolist())
    comp = sum(i.compress_size for i in z.infolist())
    big = sorted(z.infolist(), key=lambda i: -i.file_size)[:3]
    print(p, "entries", len(z.infolist()), "uncompressed MB %.1f" % (tot/1e6), "compressed MB %.1f" % (comp/1e6), "ratio %.2f" % (tot/comp))
    for i in big: print("   ", i.filename, "%.1f MB" % (i.file_size/1e6))
