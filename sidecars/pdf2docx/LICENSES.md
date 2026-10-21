# Licences in the pdf2docx sidecar image

The image as a whole is distributed under **AGPL-3.0** terms because the glue (`server.py`, `worker.py`) imports
PyMuPDF. Our own files are MIT OR Apache-2.0, which is compatible. The product talks to the image only over HTTP, as a
separate program; nothing from this image is linked into or vendored in our code. We never modify PyMuPDF or MuPDF.

Full licence texts: every Python package keeps its own `LICENSE*` files in `site-packages/<name>-<ver>.dist-info/`
inside the image (`docker run --rm <image> sh -c 'ls /usr/local/lib/python3.12/site-packages/*.dist-info/licenses'`);
the AGPL-3.0 text of PyMuPDF is saved with the source bundle (`scripts/sidecar-sources.sh`, file `PyMuPDF-COPYING.txt`).

| Component | Version | Licence | Notes |
|---|---|---|---|
| `server.py`, `worker.py`, `relay.py`, `tests/` | this repo | MIT OR Apache-2.0 | our code |
| `merge_lines.py` | sha256 `9563dd3a...` | MIT OR Apache-2.0 | owner's code (decision O4), copied unmodified |
| pdf2docx | 0.5.13 | MIT | MIT since 0.5.12; earlier releases were GPL/AGPL, so the pin must not go below 0.5.12 |
| PyMuPDF | 1.26.7 | **AGPL-3.0** (or Artifex commercial) | wheel bundles MuPDF 1.26.12 (AGPL-3.0); source offer in SOURCES.md |
| python-docx | 1.2.0 | MIT | |
| lxml | 6.1.3 | BSD-3-Clause | |
| numpy | 2.5.4 | BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0 | wheel bundles libgfortran/libquadmath (GPL-3 with GCC runtime exception) |
| fonttools | 4.67.0 | MIT | |
| fire | 0.7.1 | Apache-2.0 | |
| termcolor | 3.3.0 | MIT | |
| typing-extensions | 4.16.0 | PSF-2.0 | |
| opencv-python-headless | 4.14.0.94 | Apache-2.0 (OpenCV) | the wheel redistributes **FFmpeg (libavcodec, libavformat, libavutil, libswscale, libswresample: LGPL-2.1+)**, libaom/libdav1d/libvpx (BSD-style), OpenSSL 1.1, OpenBLAS (BSD-3). The LGPL libraries are shared objects, unmodified, replaceable. No x264/x265 (GPL) is present. |
| python:3.12-slim-bookworm | digest in `Dockerfile` | Python PSF-2.0; Debian packages under their own licences (GPL/LGPL/MIT/...) | unmodified base image; Debian sources are available from Debian |

PyMuPDF 2.x is deliberately not used: it depends on `pymupdf-layout` and `pymupdf4llm`, whose licences are not verified
here, and on onnxruntime.
