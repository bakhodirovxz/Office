"""Run inside the image (see test.sh). Fixtures are synthetic: flat-ODF text written here, turned into
docx/xlsx/pptx by soffice itself. No corpus files."""
import http.client, json, os, re, subprocess, sys, tempfile, threading, time, unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import server

NS = ('xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
      'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
      'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
      'xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" '
      'xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" '
      'xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" '
      'office:version="1.2" office:mimetype="application/vnd.oasis.opendocument.%s"')
TXT = "Oʻzbekiston Ўзбекистон sinov matni"  # U+02BB and Cyrillic


def flat(kind, body):
    return '<?xml version="1.0" encoding="UTF-8"?><office:document %s><office:body>%s</office:body></office:document>' % (
        NS % kind, body)


FIXTURES = {
    "fodt": flat("text", "<office:text>%s</office:text>" % "".join("<text:p>%s %d</text:p>" % (TXT, i) for i in range(5))),
    "fods": flat("spreadsheet", "<office:spreadsheet><table:table table:name=\"S\">"
                 "<table:table-row><table:table-cell office:value-type=\"float\" office:value=\"2\"/>"
                 "<table:table-cell office:value-type=\"float\" office:value=\"3\"/>"
                 "<table:table-cell table:formula=\"of:=[.A1]*[.B1]\" office:value-type=\"float\" office:value=\"6\"/>"
                 "</table:table-row></table:table></office:spreadsheet>"),
    "fodp": flat("presentation", "<office:presentation>%s</office:presentation>" % "".join(
        '<draw:page draw:name="s%d"><draw:frame svg:x="2cm" svg:y="2cm" svg:width="20cm" svg:height="3cm">'
        "<draw:text-box><text:p>Slide %d %s</text:p></draw:text-box></draw:frame></draw:page>" % (i, i, TXT)
        for i in range(1, 6))),
}
TARGET = {"fodt": "docx", "fods": "xlsx", "fodp": "pptx"}
FILES = {}  # kind -> bytes


def soffice(tmp, src, to, name):
    subprocess.run(["soffice", "-env:UserInstallation=file://%s/p-%s" % (tmp, name), "--headless", "--norestore",
                    "--convert-to", to, "--outdir", tmp, src], check=True, capture_output=True, timeout=120)


def setUpModule():
    tmp = tempfile.mkdtemp(prefix="fix-")
    for ext, xml in FIXTURES.items():
        p = os.path.join(tmp, "f." + ext)
        open(p, "w", encoding="utf-8").write(xml)
        soffice(tmp, p, TARGET[ext], ext)
        FILES[TARGET[ext]] = open(os.path.join(tmp, "f." + TARGET[ext]), "rb").read()


def start(**over):
    cfg = server.default_config()
    cfg.update(host="127.0.0.1", port=0, **over)
    s = server.make_server(cfg)
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return s


def post(s, to, data):
    c = http.client.HTTPConnection("127.0.0.1", s.server_address[1], timeout=180)
    c.request("POST", "/v1/convert?to=" + to, body=data)
    r = c.getresponse()
    return r.status, r.read()


def pages(pdf):
    return len(re.findall(rb"/Type\s*/Page[^s]", pdf))


class T(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = start()

    @classmethod
    def tearDownClass(cls):
        cls.s.shutdown()

    def test_healthz_and_fonts(self):
        c = http.client.HTTPConnection("127.0.0.1", self.s.server_address[1])
        c.request("GET", "/healthz")
        h = json.loads(c.getresponse().read())
        self.assertIn("7.4.7", h["versions"]["libreoffice"])
        self.assertEqual(h["versions"]["libreoffice-impress"], "4:7.4.7-1+deb12u14")
        # no Microsoft fonts; Calibri must map to the metric-compatible Carlito; U+02BB must be covered
        fam = subprocess.run(["fc-list", ":", "family"], capture_output=True, text=True).stdout
        self.assertNotRegex(fam, r"Calibri|Cambria|Arial|Times New Roman")
        self.assertIn("Carlito", subprocess.run(["fc-match", "Calibri"], capture_output=True, text=True).stdout)
        self.assertIn("Caladea", subprocess.run(["fc-match", "Cambria"], capture_output=True, text=True).stdout)
        self.assertTrue(subprocess.run(["fc-list", ":charset=2bb 42b"], capture_output=True, text=True).stdout.strip())

    def test_to_pdf(self):
        for kind in ("docx", "xlsx", "pptx"):
            st, body = post(self.s, "pdf", FILES[kind])
            self.assertEqual(st, 200, kind)
            self.assertTrue(body.startswith(b"%PDF-"), kind)

    def test_five_slides_five_pages(self):
        st, body = post(self.s, "pdf", FILES["pptx"])
        self.assertEqual((st, pages(body)), (200, 5))

    def test_png_and_roundtrip(self):
        st, body = post(self.s, "png", FILES["pptx"])
        self.assertEqual((st, body[:4]), (200, b"\x89PNG"))
        st, body = post(self.s, "docx", FILES["docx"])
        self.assertEqual((st, body[:2]), (200, b"PK"))

    def test_cross_kind_refused(self):
        st, body = post(self.s, "xlsx", FILES["docx"])
        self.assertEqual((st, json.loads(body)["code"]), (422, "unsupported_conversion"))

    def test_corrupt_input_clear_error(self):
        bad = FILES["docx"][:200] + b"\x00" * 50  # truncated zip
        st, body = post(self.s, "pdf", bad)
        self.assertEqual((st, json.loads(body)["code"]), (400, "bad_input"))
        # a valid zip whose document part is garbage reaches soffice, which cannot load it
        import io, zipfile
        b = io.BytesIO()
        with zipfile.ZipFile(b, "w") as z:
            z.writestr("word/document.xml", "not xml at all")
        st, body = post(self.s, "pdf", b.getvalue())
        self.assertEqual((st, json.loads(body)["code"]), (422, "bad_input"))
        self.assertIn("could not", json.loads(body)["message"])

    def test_legacy_ole_refused(self):
        st, body = post(self.s, "pdf", b"\xd0\xcf\x11\xe0" + b"\x00" * 600)
        self.assertEqual((st, json.loads(body)["code"]), (415, "legacy_format"))

    def test_two_parallel_jobs(self):
        res = []
        ts = [threading.Thread(target=lambda k=k: res.append(post(self.s, "pdf", FILES[k]))) for k in ("docx", "pptx")]
        [t.start() for t in ts]
        [t.join() for t in ts]
        self.assertEqual([r[0] for r in res], [200, 200])
        self.assertTrue(all(r[1].startswith(b"%PDF-") for r in res))


class Timeout(unittest.TestCase):
    def test_timeout_kills_the_process_group(self):
        pidfile = tempfile.mktemp(prefix="pid-")
        os.environ["SLOW_PID_FILE"] = pidfile
        s = start(timeout_s=2, worker=os.path.join(os.path.dirname(__file__), "slow_worker.py"))
        try:
            t0 = time.time()
            st, body = post(s, "pdf", FILES["docx"])
            self.assertEqual((st, json.loads(body)["code"]), (504, "timeout"))
            self.assertLess(time.time() - t0, 20)
            pid = int(open(pidfile).read())
            time.sleep(0.5)
            try:
                state = open("/proc/%d/stat" % pid).read().rsplit(")", 1)[1].split()[0]
            except FileNotFoundError:
                state = "gone"
            self.assertIn(state, ("gone", "Z"))  # the grandchild must not survive
        finally:
            s.shutdown()


if __name__ == "__main__":
    unittest.main()
