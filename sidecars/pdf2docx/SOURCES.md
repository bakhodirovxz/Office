# Source offer for the AGPL components (pdf2docx sidecar image)

The image contains PyMuPDF and the MuPDF library inside its wheel. Both are AGPL-3.0 and are used **unmodified**.
Whoever receives the image can get the exact corresponding source:

| Component | Version | Source | sha256 |
|---|---|---|---|
| PyMuPDF | 1.26.7 | https://files.pythonhosted.org/packages/48/d6/09b28f027b510838559f7748807192149c419b30cb90e6d5f0cf916dc9dc/pymupdf-1.26.7.tar.gz | `71add8bdc8eb1aaa207c69a13400693f06ad9b927bea976f5d5ab9df0bb489c3` (published by PyPI) |
| MuPDF | 1.26.12 | https://mupdf.com/downloads/archive/mupdf-1.26.12-source.tar.gz | `6baf910928f404167ba49be6340195dec340795724722b331f5a2143f5aa0d01` (computed by us, 2026-10-21; Artifex publishes none) |

`scripts/sidecar-sources.sh` downloads both, checks the PyPI hash, and writes `MANIFEST.txt` plus the AGPL text.
Keep that directory next to every image tag that is distributed (Phase 10). Phase 3 only builds locally.

Other pinned Python packages are permissive (see LICENSES.md); their sdists are on PyPI under the versions and hashes in
`requirements.lock`. The FFmpeg libraries inside the opencv wheel are LGPL; their source is at
https://ffmpeg.org/ (the wheel's `LICENSE-3RD-PARTY.txt` names the exact build).

Rules that keep the AGPL section-13 network clause out of play: no patch to PyMuPDF or MuPDF, ever; if one is ever
needed, stop and ask the owner, because the modified source must then be offered to every network user.

## Reproducing the build

1. `sidecars/pdf2docx/fetch-wheels.sh` downloads the wheels in `requirements.lock` (pip verifies every sha256).
   This is done on the host because the Docker build network on the dev PC sits behind a TLS-intercepting proxy.
2. `docker build -t office/pdf2docx:dev sidecars/pdf2docx` installs them with `--no-index --require-hashes`.
3. `sidecars/pdf2docx/test.sh` runs the unittest suite inside the image (no network).
4. `sidecars/pdf2docx/run.sh` starts it with limits (2 GB, 2 CPUs, 128 pids, read-only root, tmpfs /tmp, internal
   network, all capabilities dropped); `http://127.0.0.1:18080/healthz`.
