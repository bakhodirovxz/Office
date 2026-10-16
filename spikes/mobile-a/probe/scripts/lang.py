import io, subprocess, sys, os
from PIL import Image, ImageChops
d = os.path.dirname(os.path.abspath(__file__))
raw = subprocess.run(["adb","exec-out","screencap","-p"], capture_output=True).stdout
im = Image.open(io.BytesIO(raw)).convert("L").crop((400, 2140, 800, 2220))
if len(sys.argv) > 2 and sys.argv[1] == "save":
    im.save(os.path.join(d, "ref-%s.png" % sys.argv[2])); print("saved")
else:
    best = None
    for n in ("en", "ru", "uz"):
        p = os.path.join(d, "ref-%s.png" % n)
        if os.path.exists(p):
            diff = sum(ImageChops.difference(im, Image.open(p)).getdata())
            if best is None or diff < best[0]: best = (diff, n)
    im.convert("RGB").save(os.path.join(d, "lang.png"))
    print(best[1] if best and best[0] < 60000 else "none"); print(best, file=sys.stderr)
