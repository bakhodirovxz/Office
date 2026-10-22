# MSVC build (Phase 4 WP13, F11)

Status: **done on this PC, 2026-10-22** (owner answer Q10: Visual Studio Community terms, open-source development).

## Toolchain
- VS 2022 Build Tools 17 (bootstrapper from `https://aka.ms/vs/17/release/vs_buildtools.exe`, Authenticode checked: Microsoft Corporation),
  components `VC.Tools.x86.x64` (MSVC 14.44.35207) and `Windows11SDK.26100`. `--installPath E:\VS\BuildTools`, about 1.6 GB on E:.
  The SDK and shared parts landed on `C:\Program Files (x86)\Windows Kits` and the installer cache: C: went 20.5 -> 16.9 GB free for the install.
  The install asked for UAC elevation once (accepted at the machine).
- `rustup toolchain install stable-x86_64-pc-windows-msvc` (minimal) and `rustup target add x86_64-pc-windows-msvc` on gnu. **The default stays gnu.**
  (That rustup run also self-updated rustup 1.29.0 -> 1.29.2.)
- Why the msvc *host* toolchain: with the gnu host, `winresource` (icon, version info) in build scripts looks for `windres`, not `rc.exe`, and the exes were built
  without icon/version resources. `scripts/build.sh --msvc` therefore exports `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc` after the overlay step.
- signtool: `C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe` (`scripts/sign.ps1` finds it).

## Build
```
scripts/build.sh all all --release --msvc        # 4 desktops, 4 CLIs, convert + convert-api; one at a time, under the build lock
scripts/build.sh services cli --release --msvc   # only convert, convert-api
scripts/ci.sh msvc                               # the same, as a CI job (on demand, in no profile)
powershell -File scripts/msvc-smoke.ps1          # start each desktop exe with a synthetic file, wait for the window, close it
```
Output: `E:/office-target/p1/msvc/<domain>/x86_64-pc-windows-msvc/release/` (`services` for the converter). Log and times: `logs/times.tsv` (suffix `-msvc`).
`--msvc` builds no web target.

## Results (release, sccache warm for most crates, 4 jobs)
| Target | Time (s) | msvc bytes | gnu bytes |
|---|---|---|---|
| docs desktop | 603 | 65 513 472 | 82 702 593 |
| docs cli | 305 | 35 598 848 | 46 122 380 |
| sheets desktop | 433 | 33 196 032 | 43 002 407 |
| sheets cli | 137 | 14 740 480 | 18 568 011 |
| slides desktop | 572 | 77 791 232 | 93 110 268 |
| slides cli | 237 | 56 254 464 | 65 599 333 |
| pdf desktop | 1536 | 67 372 032 | 67 534 896 |
| pdf cli | 617 | 42 353 152 | 41 806 701 |
| convert | 250 (with convert-api) | 3 021 312 | 5 926 370 |
| convert-api | | 6 732 288 | 12 645 877 |

First (cold, gnu-host) attempt took 885 s for slides desktop; times above are the second pass with the msvc host. msvc exes are 0 to 22 % smaller (pdf about equal).
**No crate failed on msvc**, no upstream patch was needed. Smoke: all four desktop exes start, show a window with the file (or the app name) and close cleanly
(docs 564 ms, sheets 2085 ms, slides 517 ms, pdf 514 ms to the window, warm).
Not done: the full GUI smoke (type, save, reopen) and WARP/RDP checks (WP15 / §7.5); `suite.exe` and the MSI do not exist yet (WP14).
