#!/usr/bin/env python3
"""Reads brand/brand.toml and writes what the APK needs from it: res/values/strings.xml (the label), assets/web/shell-config.json
(the same JSON suite-serve's brand_json builds, with local=true) and the manifest (MIME filters). Prints the application id and version code.
Usage: gen-brand.py <brand.toml> <manifest-template> <out-dir>   (our code, MIT OR Apache-2.0)"""
import json, sys, tomllib
from pathlib import Path
from xml.sax.saxutils import escape

MIMES = ["application/vnd.openxmlformats-officedocument.wordprocessingml.document", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
         "application/vnd.openxmlformats-officedocument.presentationml.presentation", "application/pdf", "application/msword", "application/vnd.ms-excel",
         "application/vnd.ms-powerpoint", "application/vnd.oasis.opendocument.text", "application/vnd.oasis.opendocument.spreadsheet",
         "application/vnd.oasis.opendocument.presentation"]
brand, tpl, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
b = tomllib.loads(brand.read_text(encoding="utf-8"))
(out / "res/values").mkdir(parents=True, exist_ok=True); (out / "assets/web").mkdir(parents=True, exist_ok=True)
(out / "res/values/strings.xml").write_text(f'<resources><string name="app_name">{escape(b["product"]["name"])}</string></resources>', encoding="utf-8")
apps = [{"id": k, "route": f"/{k}/", "prefix": b["apps"][k]["upstream"], "names": b["apps"][k]["names"]} for k in ("docs", "sheets", "slides", "pdf")]
cfg = {"product": b["product"]["name"], "local": True, "accent": {"light": b["colors"]["accent_light"], "dark": b["colors"]["accent_dark"]},
       "apps": apps, "convert": {"route": "/convert/", "names": b["apps"]["convert"]["names"]}}
(out / "assets/web/shell-config.json").write_text(json.dumps(cfg, ensure_ascii=False), encoding="utf-8")
data = "".join(f'<data android:mimeType="{m}" />' for m in MIMES)
(out / "AndroidManifest.xml").write_text(tpl.read_text(encoding="utf-8").replace("@MIMES@", data), encoding="utf-8")
a = b["android"]
print(a["application_id"], a["version_code"], b["product"]["version"])
