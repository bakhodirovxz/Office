import sys, json, os, socket, time
# usage: glide.py LAYOUT WORD [step_ms]  -- injects a glide through the emulator console (fast, in-process)
d = os.path.dirname(os.path.abspath(__file__))
layout, word = sys.argv[1], sys.argv[2]
step_ms = int(sys.argv[3]) if len(sys.argv) > 3 else 12
EN = {}
for i, c in enumerate("qwertyuiop"): EN[c] = (58 + 107.1 * i, 1718)
for i, c in enumerate("asdfghjkl"): EN[c] = (111 + 107.1 * i, 1869)
for i, c in enumerate("zxcvbnm"): EN[c] = (218 + 107.1 * i, 2022)
m = EN if layout == "en" else {k: tuple(v) for k, v in json.load(open(os.path.join(d, layout + ".json"), encoding="utf-8")).items()}
pts = [m[c] for c in word]
path = [pts[0]]
for p in pts[1:]:
    if p != path[-1]:
        path.append(p)
dense = []
for a, b in zip(path, path[1:]):
    n = max(6, int(((a[0]-b[0])**2 + (a[1]-b[1])**2) ** 0.5 / 18))
    for k in range(n):
        dense.append((a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n))
dense.append(path[-1])
# dwell on the final letter
dense += [path[-1]] * 6

CRLF = chr(13) + chr(10)
tok = open(os.path.expanduser("~/.emulator_console_auth_token")).read().strip()
sock = socket.create_connection(("127.0.0.1", 5554))
sock.settimeout(2)


def send(c):
    sock.sendall((c + CRLF).encode())
    try:
        return sock.recv(4096)
    except Exception:
        return b""


try:
    sock.recv(4096)
except Exception:
    pass
send("auth " + tok)


def raw(x, y):
    return int(x / 1080 * 32767), int(y / 2400 * 32767)


def ev(t, c, v):
    return send("event send %d:%d:%d" % (t, c, v))


x, y = raw(*dense[0])
ev(3, 47, 0); ev(3, 57, 1234); ev(3, 53, x); ev(3, 54, y); ev(3, 58, 100); ev(1, 330, 1)
r = ev(0, 0, 0)
print("first reply:", r[:80])
time.sleep(0.1)
t0 = time.time()
for p in dense[1:]:
    x, y = raw(*p)
    ev(3, 53, x); ev(3, 54, y); ev(0, 0, 0)
    time.sleep(step_ms / 1000.0)
time.sleep(0.15)
ev(3, 57, -1); ev(1, 330, 0); ev(0, 0, 0)
print("glide pts", len(dense), "secs", round(time.time() - t0, 2))
sock.close()
