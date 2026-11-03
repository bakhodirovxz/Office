#!/usr/bin/env python3
"""Packs the page-host test plugin of ci/plugins/webhost-test (WP5) into install-from-file packages for the Android run (WP9):
test-host (all eight commands, a network permission it can never use on Android), test-bad-import (M1), test-bad-export (M2) and test-nonet (M3: no network in the manifest), test-own-memory and test-gc-type (M2b), and test-host-changed (F6: id test.host with another manifest, to reinstall over test.host).
Usage: make-test-packages.py <out-dir>   (our code, MIT OR Apache-2.0)"""
import sys, zipfile
from pathlib import Path

src = Path(__file__).resolve().parents[3] / "ci/plugins/webhost-test"
out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
CMDS = ["hello", "net", "spin", "grow", "xss", "edit", "deny", "many"]

def toml(pid, name, net='["https://example.org"]'):
    t = f'id = "{pid}"\nversion = "1.0.0"\napi = 1\napps = ["docs"]\npublisher = "Test publisher"\nname.en = "{name}"\nname.ru = "{name}"\nname.uz-Latn = "{name}"\nname.uz-Cyrl = "{name}"\n\n'
    t += f'[permissions]\ndocument = "write"\nnetwork = {net}\nmemory_mb = 2\n\n'
    for c in CMDS:
        t += f'[[command]]\nid = "{c}"\nlabel.en = "Cmd {c}"\nreads = ["none"]\nparams = {{}}\n\n'
    return t + '[[panel]]\nid = "p"\ntitle.en = "Panel"\n'

for pid, name, wasm in [("test.host", "Test plugin", "plugin.wasm"), ("test.bad-import", "Bad import", "bad-import.wasm"), ("test.bad-export", "Bad export", "bad-export.wasm"), ("test.nonet", "No network", "plugin.wasm"), ("test.own-memory", "Own memory", "own-memory.wasm"), ("test.gc-type", "GC type", "gc-type.wasm"), ("test.host-changed", "Test plugin changed", "plugin.wasm")]:
    with zipfile.ZipFile(out / f"{pid}-1.0.0.zip", "w", zipfile.ZIP_STORED) as z:
        for n, b in (("plugin.toml", toml(pid.replace('-changed', ''), name, '[]' if pid == 'test.nonet' else '["https://example.org"]').encode()), ("plugin.wasm", (src / wasm).read_bytes())):
            z.writestr(zipfile.ZipInfo(n, (1980, 1, 1, 0, 0, 0)), b)
    print(out / f"{pid}-1.0.0.zip")
