import sys, io, subprocess
from PIL import Image
name = sys.argv[1]
scale = float(sys.argv[2]) if len(sys.argv) > 2 else 0.6
raw = subprocess.run(["adb","exec-out","screencap","-p"], capture_output=True).stdout
im = Image.open(io.BytesIO(raw)).convert("RGB")
im = im.resize((int(im.width*scale), int(im.height*scale)), Image.LANCZOS)
out = "C:/Users/uge226/Desktop/office/docs/phase0/mobile-a/%s.jpg" % name
im.save(out, quality=72)
import os; print(out, os.path.getsize(out))
