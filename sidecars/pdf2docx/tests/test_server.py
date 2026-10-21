"""Run inside the image: docker run --rm office/pdf2docx:dev python -m unittest discover -s /app/tests -v
Every PDF is generated here from scratch (PyMuPDF); no corpus files."""
import http.client, io, json, os, sys, tempfile, threading, time, unittest, zipfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import fitz
import server

BODY = ("Бу ҳужжат синов учун яратилган. Ҳар бир сатр алоҳида ёзилган, лекин бир жумла\n"
        "давом этади ва нуқта билан тугайди. Иккинчи жумла ҳам шундай\n"
        "бошланади ва келгуси сатрда давом этади.")


def text_pdf(pages=1, text=BODY):
    doc = fitz.open()
    for _ in range(pages):
        page = doc.new_page()
        tw = fitz.TextWriter(page.rect)  # embeds a Unicode font; insert_text(fontname="tiro") loses Cyrillic
        font = fitz.Font("tiro")
        y = 72
        for line in text.split("\n"):
            tw.append((60, y), line, font=font, fontsize=11)
            y += 16
        tw.write_text(page)
    return doc.tobytes()


def scanned_pdf():
    """Image-only page: render a text page to a bitmap and embed that, so there is no text layer."""
    pix = fitz.open(stream=text_pdf(), filetype="pdf")[0].get_pixmap(dpi=50)
    doc = fitz.open()
    doc.new_page().insert_image(fitz.Rect(0, 0, 595, 842), pixmap=pix)
    return doc.tobytes()


def encrypted_pdf():
    return fitz.open(stream=text_pdf(), filetype="pdf").tobytes(
        encryption=fitz.PDF_ENCRYPT_AES_256, user_pw="u", owner_pw="o")


SLEEPER = "import time; time.sleep(60)\n"
CRASHER = "import os, signal; os.kill(os.getpid(), signal.SIGKILL)\n"


def serve(**over):
    cfg = server.default_config()
    cfg.update(host="127.0.0.1", port=0, **over)
    s = server.make_server(cfg, vers={"test": True})
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return s


def call(s, method, path, body=None):
    c = http.client.HTTPConnection("127.0.0.1", s.server_address[1], timeout=120)
    c.request(method, path, body=body)
    r = c.getresponse()
    data = r.read()
    c.close()
    return r.status, r, data


def code(data):
    return json.loads(data)["code"]


class Real(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.s = serve(max_pages=2, max_bytes=2_000_000)

    @classmethod
    def tearDownClass(cls):
        cls.s.shutdown()

    def post(self, body, q=""):
        return call(self.s, "POST", "/v1/pdf2docx" + q, body)

    def test_healthz_reports_versions_and_slots(self):
        st, _, d = call(self.s, "GET", "/healthz")
        j = json.loads(d)
        self.assertEqual((st, j["ok"], j["busy"], j["slots"]), (200, True, 0, 1))

    def test_real_versions_are_the_pins(self):
        v = server.versions()
        self.assertEqual(v["pdf2docx"], "0.5.13")
        self.assertEqual(v["pymupdf"], "1.26.7")
        self.assertEqual(len(v["merge_lines"]), 8)
        self.assertFalse(v["merge_lines"].endswith("MODIFIED"))

    def test_ok_conversion_keeps_cyrillic_and_reports(self):
        st, r, d = self.post(text_pdf())
        self.assertEqual(st, 200, d[:200])
        self.assertIn("wordprocessingml", r.getheader("Content-Type"))
        rep = json.loads(r.getheader("X-Convert-Report"))
        self.assertEqual(rep["pages"], 1)
        self.assertGreaterEqual(rep["merged"], 0)
        self.assertGreater(rep["paragraphs"], 0)
        xml = zipfile.ZipFile(io.BytesIO(d)).read("word/document.xml").decode()
        self.assertIn("синов", xml)

    def test_merge_off_reports_zero_merged(self):
        st, r, _ = self.post(text_pdf(), "?merge=0")
        self.assertEqual(st, 200)
        self.assertEqual(json.loads(r.getheader("X-Convert-Report"))["merged"], 0)

    def test_pages_selection(self):
        st, r, _ = self.post(text_pdf(2), "?pages=2")
        self.assertEqual((st, json.loads(r.getheader("X-Convert-Report"))["pages"]), (200, 1))

    def test_400_bad_pages(self):
        for q in ("?pages=0", "?pages=3", "?pages=x", "?pages=2-1"):
            st, _, d = self.post(text_pdf(2), q)
            self.assertEqual((st, code(d)), (400, "bad_request"), q)

    def test_400_not_a_pdf_empty_and_truncated(self):
        for body in (b"hello", b"", text_pdf()[:15]):
            st, _, d = self.post(body)
            self.assertEqual((st, code(d)), (400, "bad_pdf"), body[:10])

    def test_413_too_large(self):
        c = http.client.HTTPConnection("127.0.0.1", self.s.server_address[1])
        c.putrequest("POST", "/v1/pdf2docx")
        c.putheader("Content-Length", "2000001")  # the server answers before reading the body
        c.endheaders()
        r = c.getresponse()
        self.assertEqual((r.status, code(r.read())), (413, "too_large"))

    def test_422_encrypted(self):
        st, _, d = self.post(encrypted_pdf())
        self.assertEqual((st, code(d)), (422, "encrypted"))

    def test_422_scanned_no_text_layer(self):
        st, _, d = self.post(scanned_pdf())
        self.assertEqual((st, code(d)), (422, "no_text_layer"))

    def test_422_too_many_pages(self):
        st, _, d = self.post(text_pdf(3))
        self.assertEqual((st, code(d)), (422, "too_many_pages"))

    def test_404_and_411(self):
        self.assertEqual(call(self.s, "GET", "/nope")[0], 404)
        self.assertEqual(call(self.s, "POST", "/nope", b"x")[0], 404)
        c = http.client.HTTPConnection("127.0.0.1", self.s.server_address[1])
        c.putrequest("POST", "/v1/pdf2docx")
        c.endheaders()
        self.assertEqual(c.getresponse().status, 411)


class FakeWorker(unittest.TestCase):
    def fake(self, src):
        self.addCleanup(lambda: os.path.exists(self.path) and os.unlink(self.path))
        f = tempfile.NamedTemporaryFile("w", suffix=".py", delete=False)
        f.write(src)
        f.close()
        self.path = f.name
        return f.name

    def test_504_timeout_kills_the_worker(self):
        s = serve(worker=self.fake(SLEEPER), timeout_s=1)
        self.addCleanup(s.shutdown)
        t = time.time()
        st, _, d = call(s, "POST", "/v1/pdf2docx", b"%PDF-1.4")
        self.assertEqual((st, code(d)), (504, "timeout"))
        self.assertLess(time.time() - t, 10)
        self.assertEqual(json.loads(call(s, "GET", "/healthz")[2])["busy"], 0)  # slot released

    def test_500_worker_crash(self):
        s = serve(worker=self.fake(CRASHER))
        self.addCleanup(s.shutdown)
        st, _, d = call(s, "POST", "/v1/pdf2docx", b"%PDF-1.4")
        self.assertEqual((st, code(d)), (500, "worker_crashed"))

    def test_503_when_busy_with_retry_after(self):
        s = serve(worker=self.fake(SLEEPER), timeout_s=4, slots=1)
        self.addCleanup(s.shutdown)
        first = []
        th = threading.Thread(target=lambda: first.append(call(s, "POST", "/v1/pdf2docx", b"%PDF-1.4")))
        th.start()
        for _ in range(50):
            if json.loads(call(s, "GET", "/healthz")[2])["busy"] == 1:
                break
            time.sleep(0.1)
        st, r, d = call(s, "POST", "/v1/pdf2docx", b"%PDF-1.4")
        self.assertEqual((st, code(d), r.getheader("Retry-After")), (503, "busy", "5"))
        th.join()
        self.assertEqual(first[0][0], 504)


if __name__ == "__main__":
    unittest.main()
