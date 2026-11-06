"""DXGI overlay / hardware-composition capabilities of every output (read-only; works on a locked PC)."""
import ctypes, ctypes.wintypes as W, uuid
def guid(s):
    b = uuid.UUID(s).bytes_le; return (ctypes.c_byte * 16).from_buffer_copy(b)
def call(obj, idx, restype, *args, argtypes=()):
    vt = ctypes.cast(obj, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))).contents
    f = ctypes.WINFUNCTYPE(restype, ctypes.c_void_p, *argtypes)(vt[idx]); return f(obj, *args)
IID_F1 = guid("770aae78-f26f-4dba-a829-253c83d1b387"); IID_O2 = guid("595e39d1-2724-4663-99b1-da969de28364"); IID_O6 = guid("068346e8-aaec-4b84-add7-137f513f77a1")
f = ctypes.c_void_p(); assert ctypes.windll.dxgi.CreateDXGIFactory1(ctypes.byref(IID_F1), ctypes.byref(f)) == 0
class ODESC(ctypes.Structure): _fields_ = [("name", ctypes.c_wchar * 32), ("rect", W.RECT), ("attached", W.BOOL), ("rot", ctypes.c_int), ("mon", W.HANDLE)]
class ADESC(ctypes.Structure): _fields_ = [("desc", ctypes.c_wchar * 128), ("ven", W.UINT), ("dev", W.UINT), ("sub", W.UINT), ("rev", W.UINT), ("vm", ctypes.c_size_t), ("sm", ctypes.c_size_t), ("ssm", ctypes.c_size_t), ("luid", ctypes.c_int64), ("flags", W.UINT)]
a = 0
while True:
    ad = ctypes.c_void_p()
    if call(f, 12, ctypes.c_long, a, ctypes.byref(ad), argtypes=(W.UINT, ctypes.c_void_p)) != 0: break
    d = ADESC(); call(ad, 10, ctypes.c_long, ctypes.byref(d), argtypes=(ctypes.c_void_p,))  # GetDesc1
    print(f"adapter {a}: {d.desc}")
    o = 0
    while True:
        out = ctypes.c_void_p()
        if call(ad, 7, ctypes.c_long, o, ctypes.byref(out), argtypes=(W.UINT, ctypes.c_void_p)) != 0: break
        od = ODESC(); call(out, 7, ctypes.c_long, ctypes.byref(od), argtypes=(ctypes.c_void_p,))
        o2 = ctypes.c_void_p(); ov = None
        if call(out, 0, ctypes.c_long, ctypes.byref(IID_O2), ctypes.byref(o2), argtypes=(ctypes.c_void_p, ctypes.c_void_p)) == 0:
            ov = call(o2, 23, W.BOOL)
        o6 = ctypes.c_void_p(); hc = None
        if call(out, 0, ctypes.c_long, ctypes.byref(IID_O6), ctypes.byref(o6), argtypes=(ctypes.c_void_p, ctypes.c_void_p)) == 0:
            fl = W.UINT(); hr = call(o6, 28, ctypes.c_long, ctypes.byref(fl), argtypes=(ctypes.c_void_p,)); hc = (hr, fl.value)
        r = od.rect
        names = [] if hc is None else [n for b, n in [(1, "FULLSCREEN"), (2, "WINDOWED"), (4, "CURSOR_STRETCHED")] if hc[1] & b]
        print(f"  output {o} {od.name} at ({r.left},{r.top})-({r.right},{r.bottom}): SupportsOverlays={bool(ov)} CheckHardwareCompositionSupport hr={hc and hex(hc[0] & 0xffffffff)} flags={hc and hc[1]} {names}")
        o += 1
    a += 1
