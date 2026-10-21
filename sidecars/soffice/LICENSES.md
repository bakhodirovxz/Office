# Licences in the soffice sidecar image

**Test-only.** This image is the LibreOffice reference renderer for the WP9 fidelity harness (owner decision O2: no
.ppt support, so the product never calls it). It runs as a separate process in its own container, reached over HTTP.
Clean-room: we compare its outputs with ours; we never copy, read or vendor LibreOffice code. Our wrapper is a plain
command-line call. Nothing from this image is linked into the product.

Licence texts are in the image under `/usr/share/doc/<package>/copyright` (Debian standard).

| Component | Version | Licence | Notes |
|---|---|---|---|
| `server.py`, `worker.py`, `relay.py`, `tests/` | this repo | MIT OR Apache-2.0 | our code |
| LibreOffice (writer, calc, impress and their core packages) | 4:7.4.7-1+deb12u14 | MPL-2.0 (some parts LGPL-3.0+ and others, per Debian `copyright`) | unmodified Debian packages; process isolation only |
| fonts-crosextra-carlito | 20220224-1 | OFL-1.1 | metric-compatible with Calibri |
| fonts-crosextra-caladea | 20200211-1 | OFL-1.1 / Apache-2.0 | metric-compatible with Cambria |
| fonts-liberation2 | 2.1.5-1 | OFL-1.1 | metric-compatible with Arial, Times New Roman, Courier New |
| fonts-noto-core | 20201225-1 | OFL-1.1 | Latin (incl. U+02BB) and Cyrillic |
| debian:bookworm-slim | digest in `Dockerfile` | Debian package licences (GPL/LGPL/MIT/...) | unmodified base image |
| python3 (Debian) | bookworm | PSF-2.0 | runs the HTTP layer |

No Microsoft fonts are installed (checked by `tests/test_server.py`).
