import sys, subprocess, json
# Gboard portrait 1080x2400 key centres, in device px. Layout files live next to this script.
EN = {}
r1 = "qwertyuiop"; r2 = "asdfghjkl"; r3 = "zxcvbnm"
for i, c in enumerate(r1): EN[c] = (58 + 107.1 * i, 1718)
for i, c in enumerate(r2): EN[c] = (111 + 107.1 * i, 1869)
for i, c in enumerate(r3): EN[c] = (218 + 107.1 * i, 2022)
EN[" "] = (590, 2178); EN["."] = (860, 2178); EN[","] = (215, 2178)
EN["<ENTER>"] = (993, 2178); EN["<BS>"] = (991, 2022); EN["<SHIFT>"] = (84, 2022); EN["<GLOBE>"] = (327, 2178)
def load(layout):
    if layout == "en": return EN
    import os
    p = os.path.join(os.path.dirname(os.path.abspath(__file__)), layout + ".json")
    return {k: tuple(v) for k, v in json.load(open(p, encoding="utf-8")).items()}
def tokens(s):
    i = 0
    while i < len(s):
        if s[i] == "<":
            j = s.index(">", i); yield s[i:j+1]; i = j + 1
        else:
            yield s[i]; i += 1
def main():
    layout, text = sys.argv[1], sys.argv[2]
    delay = sys.argv[3] if len(sys.argv) > 3 else "0.25"
    m = load(layout)
    cmds = []
    for t in tokens(text):
        x, y = m[t]
        cmds.append("input tap %d %d; sleep %s" % (x, y, delay))
    subprocess.run(["adb", "shell", "; ".join(cmds)])
main()
