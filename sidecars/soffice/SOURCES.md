# Sources and reproducing the soffice image

All components are unmodified Debian bookworm packages. Source for any of them:
`apt-get source <package>=<version>` against `deb-src http://deb.debian.org/debian bookworm main`, or
https://sources.debian.org/ (LibreOffice: package `libreoffice`, version `4:7.4.7-1+deb12u14`).
MPL-2.0 has no source-offer obligation for unmodified binaries beyond pointing at the source, which is this file.

## Build

`docker build -t office/soffice:dev sidecars/soffice`

Proxy note: Docker on the dev PC sits behind a TLS-intercepting proxy, so https downloads inside a build fail. apt is
not affected: it uses plain http to deb.debian.org and verifies the GPG signatures of the Release files, so a
tampered package would be rejected. No host-side fetch was needed.

Pins: the base image by digest; the LibreOffice and font packages by exact version. Debian's security team replaces
`libreoffice` versions on bookworm-security; when the pinned `deb12u14` is gone from the mirror the build fails with
"version not found". Then point apt at `http://snapshot.debian.org/archive/debian-security/<timestamp>/` for that
build, or bump the pins and re-run the fidelity baselines.

## Run and test

- `sidecars/soffice/test.sh` runs the suite in the image: read-only root, tmpfs /tmp, `--network none`, 2 GB, 2 CPUs.
- `sidecars/soffice/run.sh` starts it on an `--internal` network (no egress) with a dev relay on
  `http://127.0.0.1:18081`.
- `POST /v1/convert?to=pdf|png|docx|xlsx|pptx` with a .docx/.xlsx/.pptx body; `GET /healthz`. `to=png` renders the
  first page only (LibreOffice's png filter). `docx|xlsx|pptx` targets are same-kind round trips.

## Measured (2026-10-21)

Image `office/soffice:dev`, id `sha256:f372c30fad2c`: 896 MB on disk (`docker images`), 242 MB compressed. `test.sh`: 9 tests, 21 s.
