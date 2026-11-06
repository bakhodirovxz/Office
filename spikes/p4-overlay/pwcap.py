"""Window flicker probe that works on a locked workstation: PrintWindow(PW_RENDERFULLCONTENT) returns
the content DWM holds for the window (the last presented swapchain buffer for a flip-model window).
Logs per grab: mean luma, fraction of near-black pixels, window rect/style; saves dark frames + neighbours.

usage: python -I pwcap.py <pid> <seconds> <outdir> [inputs]
  inputs: comma list of synthetic window messages to post while capturing: type, scroll, click
"""
import ctypes, ctypes.wintypes as W, sys, time, os, collections, random
import numpy as np
from PIL import Image

import subprocess
arg1, secs, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
if arg1.isdigit():
    pid = int(arg1)
else:  # launch: "exe|arg|arg" and capture from the first visible window
    parts = arg1.split("|"); proc = subprocess.Popen(parts, cwd=os.path.dirname(parts[0]) or None); pid = proc.pid
    open(out + ".pid", "w").write(str(pid)) if os.path.isdir(os.path.dirname(out) or ".") else None
    t_launch = time.perf_counter()
inputs = sys.argv[4].split(",") if len(sys.argv) > 4 else []
os.makedirs(out, exist_ok=True)
u32, g32 = ctypes.windll.user32, ctypes.windll.gdi32
u32.SetProcessDPIAware()
u32.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]

def main_window():
    found = []
    @ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    def cb(h, _):
        p = W.DWORD(); u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        c = ctypes.create_unicode_buffer(64); u32.GetClassNameW(h, c, 64)
        if p.value == pid and c.value == "Window Class": found.append(h)
        return True
    u32.EnumWindows(cb, 0)
    return found[0]

class BMI(ctypes.Structure):
    _fields_ = [("a", W.DWORD), ("w", W.LONG), ("h", W.LONG), ("p", W.WORD), ("bc", W.WORD), ("c", W.DWORD),
                ("s", W.DWORD), ("x", W.LONG), ("y", W.LONG), ("u", W.DWORD), ("i", W.DWORD)]

while True:
    try:
        hwnd = main_window(); break
    except IndexError:
        time.sleep(0.01)
print("window after", round((time.perf_counter() - t_launch) * 1000) if 't_launch' in dir() else 0, "ms", "pid", pid)
dc = u32.GetDC(hwnd); mdc = g32.CreateCompatibleDC(dc)
cur = None
def grab():
    global cur
    r = W.RECT(); u32.GetWindowRect(hwnd, ctypes.byref(r)); w, h = r.right - r.left, r.bottom - r.top
    if cur is None or cur[0] != (w, h):
        bmp = g32.CreateCompatibleBitmap(dc, w, h); g32.SelectObject(mdc, bmp)
        cur = ((w, h), bmp, (ctypes.c_ubyte * (w * h * 4))(), BMI(ctypes.sizeof(BMI), w, -h, 1, 32, 0, 0, 0, 0, 0, 0))
    _, bmp, buf, bmi = cur
    u32.PrintWindow(hwnd, mdc, 2)
    g32.GetDIBits(mdc, bmp, 0, h, buf, ctypes.byref(bmi), 0)
    a = np.frombuffer(buf, np.uint8).reshape(h, w, 4)[:, :, 2::-1]
    return a, (r.left, r.top, w, h), u32.GetWindowLongW(hwnd, -16) & 0xffffffff

WM_CHAR, WM_KEYDOWN, WM_MOUSEWHEEL, WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP = 0x102, 0x100, 0x20A, 0x200, 0x201, 0x202
def poke(kind, rect):
    x, y = rect[2] // 2, rect[3] // 2
    if kind == "type":
        ch = random.choice("abcdefgh ")
        u32.PostMessageW(hwnd, WM_CHAR, ord(ch), 1)
    elif kind == "scroll":
        sx, sy = rect[0] + x, rect[1] + y
        u32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, (y << 16) | x)
        u32.PostMessageW(hwnd, WM_MOUSEWHEEL, ((-120 if random.random() < .6 else 120) & 0xffff) << 16, (sy << 16) | sx)
    elif kind == "click":
        cx, cy = x, y
        u32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, (cy << 16) | cx)
        u32.PostMessageW(hwnd, WM_LBUTTONDOWN, 1, (cy << 16) | cx); u32.PostMessageW(hwnd, WM_LBUTTONUP, 0, (cy << 16) | cx)

log = open(os.path.join(out, "frames.tsv"), "w")
log.write("t_ms\tluma\tdark_frac\tx\ty\tw\th\tstyle\tvisible\n")
ring = collections.deque(maxlen=3); hist = collections.deque(maxlen=30)
pending = 0; saved = 0; n = 0; events = []; last_geom = None; geom_changes = 0; next_poke = 0.0
t0 = time.perf_counter()
while (now := time.perf_counter() - t0) < secs:
    if inputs and now >= next_poke:
        try:
            poke(random.choice(inputs), last_geom[1] if last_geom else (0, 0, 800, 600))
        except Exception:
            pass
        next_poke = now + 0.15
    f, rect, style = grab(); n += 1
    small = f[::8, ::8].astype(np.float32)
    luma = small.mean(axis=2)
    wl = float(luma.mean()); dark = float((luma < 40).mean())
    if last_geom is not None and (rect, style) != last_geom: geom_changes += 1
    last_geom = (rect, style)
    log.write(f"{now*1000:.1f}\t{wl:.1f}\t{dark:.3f}\t{rect[0]}\t{rect[1]}\t{rect[2]}\t{rect[3]}\t{style:#x}\t{u32.IsWindowVisible(hwnd)}\n")
    med = float(np.median(hist)) if len(hist) >= 10 else None
    if (med is not None and med - wl > 20) or (dark > 0.5 and saved < 60):
        events.append(now * 1000)
        if saved < 40:
            for tt, ff in ring:
                Image.fromarray(ff).save(os.path.join(out, f"ev{len(events):03d}_pre_{tt:.0f}.png")); saved += 1
            Image.fromarray(np.ascontiguousarray(f[::2, ::2])).save(os.path.join(out, f"ev{len(events):03d}_hit_{now*1000:.0f}.png")); saved += 1
        pending = 1
    elif pending and saved < 60:
        Image.fromarray(np.ascontiguousarray(f[::2, ::2])).save(os.path.join(out, f"ev{len(events):03d}_post_{now*1000:.0f}.png")); saved += 1; pending = 0
    ring.append((now * 1000, np.ascontiguousarray(f[::2, ::2]))); hist.append(wl)
log.close()
el = time.perf_counter() - t0
print(f"grabs={n} secs={el:.1f} rate={n/el:.1f}/s dark_events={len(events)} geom_style_changes={geom_changes} at_ms={[round(e) for e in events[:20]]}")
