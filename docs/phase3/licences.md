# Phase 3 licence table (WP5, PDF to DOCX sidecar)

Status: **awaiting architect sign-off** (WP5 done-check, PLAN section 3.1). Facts verified 2026-10-21 against the
wheels' own metadata (the pinned files in `sidecars/pdf2docx/requirements.lock`). Not legal advice.

| Component | Licence | Where | Verdict |
|---|---|---|---|
| pdf2docx 0.5.13 | MIT (wheel metadata) | sidecar image | allowed; pin must stay >= 0.5.12 |
| PyMuPDF 1.26.7 + MuPDF 1.26.12 | AGPL-3.0 or Artifex commercial | sidecar image only | allowed only as an isolated container reached over HTTP; never linked, never vendored; source offer in `sidecars/pdf2docx/SOURCES.md`, bundle by `scripts/sidecar-sources.sh` |
| python-docx 1.2.0, fonttools 4.67.0, termcolor 3.3.0 | MIT | image | allowed |
| lxml 6.1.3, numpy 2.5.4 | BSD-3-Clause (numpy: plus 0BSD, MIT, Zlib, CC0) | image | allowed |
| fire 0.7.1 | Apache-2.0 | image | allowed |
| typing-extensions 4.16.0 | PSF-2.0 | image | allowed |
| opencv-python-headless 4.14.0.94 | Apache-2.0; **wheel bundles FFmpeg libs (LGPL-2.1+)**, libaom/dav1d/libvpx, OpenSSL 1.1 | image | allowed in the container; resolves the PLAN's "UNVERIFIED" item: FFmpeg is present (libavcodec, libavformat, libavutil, libswscale, libswresample), no x264/x265; LGPL objects are unmodified shared libraries |
| `merge_lines.py` | MIT OR Apache-2.0 (owner decision O4) | image, byte-identical copy (sha256 `9563dd3a...`, checked at build and shown in `/healthz`) | allowed |
| `server.py`, `worker.py`, `relay.py`, tests | MIT OR Apache-2.0 (ours) | image | allowed; the image as a whole is AGPL-3.0 because the worker imports PyMuPDF |
| python:3.12-slim-bookworm (digest-pinned) | PSF-2.0 + Debian package licences | image base | allowed; unmodified |
| LibreOffice 7.4.7 (Debian `libreoffice-writer/calc/impress`) | MPL-2.0 (+ LGPL-3.0+ parts) | `sidecars/soffice` image, **test-only** reference renderer (WP6 reduced, O2 = no .ppt) | allowed as a separate process over HTTP; nothing linked or vendored; we compare outputs and never copy code; see `sidecars/soffice/LICENSES.md` |
| Carlito, Caladea, Liberation 2, Noto Core fonts (Debian) | OFL-1.1 (Caladea also Apache-2.0) | soffice image | allowed; no Microsoft fonts |

## Findings the architect should look at

1. **PyMuPDF is pinned at 1.26.7, not newer.** pdf2docx needs >= 1.26.7. PyMuPDF 2.0 adds hard dependencies on
   `pymupdf-layout`, `pymupdf4llm` and onnxruntime; their licences were not checked, so 1.26.7 (the floor) avoids them.
   Newer 1.27/1.28 were not evaluated.
2. **LGPL FFmpeg inside the opencv wheel** was not in the PLAN's table. Fine for a container, but it widens the
   notices needed at Phase 10 distribution (ship `cv2/LICENSE-3RD-PARTY.txt`, already inside the image).
3. **MuPDF has no published checksum**; `scripts/sidecar-sources.sh` records the sha256 it computed.
4. **Wheels are fetched on the host** (pip, hashes enforced), not in `docker build`, because the dev PC's Docker network
   sits behind a TLS-intercepting proxy. `wheelhouse/` is git-ignored. Hash-verified, so the result is the same.
5. The AGPL section-13 clause is only relevant to modified versions. No patches exist and the rule is written down.
