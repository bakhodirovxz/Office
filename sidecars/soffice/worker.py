"""One LibreOffice job in a child process: python3 -I worker.py in.<ext> outdir opts.json

Each job gets its own profile (-env:UserInstallation); one shared profile makes concurrent conversions fail.
stdout: one JSON line. {"ok":true,"file":"..."} or {"ok":false,"status":N,"code":"...","message":"..."}.
"""
import glob, json, os, subprocess, sys


def fail(status, code, message):
    print(json.dumps({"ok": False, "status": status, "code": code, "message": message}))
    sys.exit(0)


def main():
    src, outdir, optpath = sys.argv[1:4]
    to = json.load(open(optpath))["to"]
    profile = os.path.join(os.path.dirname(os.path.abspath(src)), "profile")
    r = subprocess.run(["soffice", "-env:UserInstallation=file://" + profile, "--headless", "--norestore",
                        "--nolockcheck", "--nodefault", "--nologo", "--convert-to", to, "--outdir", outdir, src],
                       capture_output=True, text=True)
    out = glob.glob(os.path.join(outdir, "*." + to))
    if r.returncode != 0 or not out:  # a corrupt file makes soffice print an error and exit 0 with no output
        sys.stderr.write("soffice rc=%s: %s %s\n" % (r.returncode, r.stdout[-300:], r.stderr[-300:]))
        fail(422, "bad_input", "LibreOffice could not load or convert the file (corrupt or unsupported)")
    print(json.dumps({"ok": True, "file": out[0]}))


main()
