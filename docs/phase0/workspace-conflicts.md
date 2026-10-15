# Phase 0 spike: can the four Craft apps share one Cargo workspace?

Evidence from real `cargo metadata` / `cargo generate-lockfile` / `cargo tree` runs (rustc 1.97.1, windows-gnu). Nothing was
compiled (no `cargo build/check/test`), so API-level breakage from version unification is NOT verified. Spike files:
`spikes/workspace/` (see its README). Upstream clones were not modified.

## 1. Verdict

**Feasible with explicit pins and a layout change; no hard dependency-resolution blocker.**

- The merged workspace (97 members: 4 apps, xtask excluded) resolves and locks: 923 lock entries, no solver errors.
- It needs a handful of explicit decisions (section 3): one value each for `version`, `rust-version`, `repository`,
  one `[patch.crates-io]` set, one lint set, one profile set, and 11 merged `[workspace.dependencies]` entries.
- One structural blocker if the upstream checkouts are kept as they are: members cannot live under another
  `[workspace]` root (`is a member of the wrong workspace`). The four root `Cargo.toml`/`Cargo.lock` files must go (the spike used
  copies with those deleted). That is the migration itself, not a dependency problem.
- No crate source or crate-level `Cargo.toml` needed editing. Third-party versions across the lockfiles are already
  nearly identical (egui/eframe/epaint/wgpu/winit/serde/quick-xml/zip/image/wasm-bindgen: same locked version everywhere).
- Duplicate-dependency count in the merged tree is essentially the union of the apps' own duplicates (32 crate names vs 33
  for the union of the four; only 4 names are new, 2 of them removable with a `[patch]`).

## 2. Static diff

### 2.1 Locked versions that differ across repos

Cargo.lock comparison of all 6 repos (739 third-party crate names; 97 appear with differing versions, mostly Windows/objc2
platform crates and the toml family). Identical in every repo that has them (no conflict): egui, eframe, epaint, egui-wgpu,
egui-winit, egui_extras (0.36.2); wgpu 30.0.1; winit 0.30.13; serde 1.0.229; serde_json 1.0.151; quick-xml 0.41.0; zip 3.0.0
(word/grid/deck only); image 0.25.10; wasm-bindgen 0.2.129; web-sys/js-sys 0.3.106; wasm-bindgen-futures 0.4.79; rayon 1.12.0;
rfd 0.17.2; arboard 3.6.1; ttf-parser 0.25.1 and rustybuzz 0.20.1 (word, deck, craft-libs only); tiny-skia 0.11.4 (pdfcraft, craft-libs only).
swash, fontdb, fontdue, cosmic-text are not used anywhere.

| Crate | wordcraft | gridcraft | deckcraft | pdfcraft | craft-libs | Same semver line? |
|---|---|---|---|---|---|---|
| tokio | 1.53.2 | 1.53.2 | 1.53.2 | 1.53.1 | 1.53.2 | y (patch drift) |
| skrifa | 0.42.1, 0.44.0, 0.47.0 | 0.44.0 | 0.42.1, 0.44.0, 0.47.0 | 0.42.1, 0.44.0, 0.47.0 | same + 0.48.0 | **N** (ws: 0.47 vs grid 0.44) |
| harfrust | 0.12.0, 0.13.3 | 0.12.0 | 0.12.0, 0.13.3 | 0.12.0 | 0.12.0 | **N** (ws 0.13; pdfcraft fonts inline 0.12) |
| read-fonts | 0.39/0.41/0.43/0.44 | 0.41 | same as word | 0.39/0.41/0.44 | + 0.45 (craft-fonts) | N |
| font-types | 0.11.3, 0.12.6 | 0.12.6 | 0.11.3, 0.12.6 | 0.11.3, 0.12.5 | 0.11.3, 0.12.6 | N |
| vello_cpu / vello_common | 0.1, 0.2 | 0.1 | 0.1, 0.2 | 0.0.8, 0.1 (+0.0.9 common) | 0.0.8, 0.1 | **N** (ws 0.2 word/deck; pdfcraft vendors vello_common) |
| kurbo | 0.13.1 | 0.13.1 | 0.13.1 | 0.11.3, 0.13.1 | 0.11.3, 0.13.1 | N (0.11 via tiny-skia/hayro) |
| tiny-skia-path | 0.12.0 | - | 0.12.0 | 0.11.4 | 0.11.4, 0.12.0 | N |
| png | 0.18.1 | 0.18.1 | 0.18.1 | 0.17.16, 0.18.1 | 0.17.16, 0.18.1 | N |
| krilla | 0.8.2 (crates.io) | - | 0.8.2 (git patch) | - | 0.8.2 (vendor path) | y, but 3 different sources |
| hayro / -syntax / -interpret | syntax 0.7.2, interpret 0.7.0 (crates.io) | - | syntax 0.7.2 | 0.7.1 / 0.7.2 / 0.7.0 (vendored forks) | vendored forks | y; source differs |
| lopdf | - | - | - | 0.45.0 (vendored) | - | n/a |
| miniz_oxide | 0.8.9, 0.9.1 | same | same | same | same | N (both lines everywhere) |
| thiserror | 1.0.69, 2.0.21 | same | same | same | same | N (both lines everywhere) |
| weezl | 0.1.12 | 0.1.12 | 0.1.12 | 0.1.12, 0.2.1 | 0.1.12 | N |
| imagesize | 0.14.0 | - | 0.14.0 | 0.13.0 | 0.13.0, 0.14.0 | N |
| base64 / rand / sha1 / sha2 | 0.22 / 0.9 / 0.11 / 0.11 | sha 0.11 | 0.22 / 0.9 | 0.22+0.23 / 0.9+0.10 / 0.10+0.11 | RustCrypto 0.10 line | N (pdfcraft/craft-libs still carry RustCrypto 0.10 for rsa/sign) |
| windows / windows-core | 0.62.2 | 0.62.2 | 0.54.0, 0.62.2 | 0.62.2 | 0.62.2 | N (deckcraft's cpal/symphonia chain pulls 0.54) |
| windows-sys | 0.52/0.59/0.60/0.61 | same | + 0.45 | 0.52/0.60/0.61 | 0.52/0.59/0.60/0.61 | N |
| uuid, zerocopy, cc, libc, either, toml* | minor drift (e.g. uuid 1.27.0 vs pdfcraft 1.26.1) | | | | | y |
| muda | - | 0.21.1 | - | 0.21.2 | 0.21.2 | y |

"y" rows collapse to one version automatically in a merged lock; "N" rows whose both lines already exist in one app stay as is.

### 2.2 `[workspace.dependencies]` keys that had to be merged (spike's choice)

| Key | wordcraft | gridcraft | deckcraft | pdfcraft | Merged value | Note |
|---|---|---|---|---|---|---|
| skrifa | 0.47 | 0.44, no-default, [std] | 0.47 | (inline 0.44 in crates) | 0.47, default features | **gridcraft jumps 0.44 -> 0.47: unverified, needs compile.** Its comment says "same version as epaint's" so it deliberately avoided a second copy; the 0.44 copy stays via epaint and pdfcraft anyway |
| harfrust | 0.13 | - | 0.13 | inline 0.12 | 0.13 | pdfcraft-fonts uses 0.12 inline: both copies stay (unifying needs a pdfcraft crate edit) |
| image | 0.25 [png,jpeg,webp,gif,bmp,tiff] | 0.25 [png,jpeg] | 0.25 [png,jpeg,webp,gif,tiff,bmp] | inline per crate | union of features | gridcraft/pdfcraft gain webp/gif |
| arboard | 3.6 no-default [wayland-data-control] | 3.6 no-default | 3.6 no-default | inline 3.6 image-data | no-default + wayland-data-control | deck/pdf gain wayland clipboard |
| egui_extras | 0.36 | 0.36 | 0.36 | 0.36 [svg] | 0.36 [svg] | |
| aes, sha1, sha2 | 0.9 / 0.11 / 0.11 | same, default-features=false | - | inline | default features on | |
| kurbo | 0.13 [serde] | - | 0.13 [serde] | 0.13 | 0.13 [serde] | |
| serde_json | [float_roundtrip] | same | same | plain | [float_roundtrip] | |
| craft-pdf | - | - | git rev c348732, =0.1.0, [write] | - | kept (git) | |
| craft-ui, craft-print, craft-quit | inline git rev d27646a | inline git rev d27646a | - | ws git revs 5c874d5 / 666c358 / f58bc1a | kept (git) | 3-4 different craft-libs revs in play (3.4) |
| keys only in one app | base64 cfb chardetng encoding_rs getrandom hmac krilla miniz_oxide regex-automata unicode-script wuff zeroize | epaint_default_fonts sys-locale | chrono i_overlay vello_common | flate2 hayro hayro-syntax lopdf weezl | copied as-is | no conflict |

Everything else in the four tables is textually identical (egui/eframe/egui_kittest 0.36, rfd 0.17, thiserror 2, serde, quick-xml 0.41,
zip 3, vello_cpu 0.2, log, proptest, rayon, regex, unicode-*).
Inline (non-workspace) requirements that disagree between repos and are NOT fixed by the merged table: pollster 0.4 (word) vs 1 (grid, pdf);
getrandom 0.3 (word) vs 0.4 (pdf); harfrust 0.12 and skrifa 0.44 (pdf); winit features x11 (word, deck) vs wayland (pdf); zbus exact `=5.19.0`
(grid, pdf, word) is fine as long as everyone keeps the same exact pin.

## 3. Other cross-repo comparisons

### 3.1 Package fields (`workspace = true` forces a single definition)

| Field | wordcraft | gridcraft | deckcraft | pdfcraft | Spike value | Consequence |
|---|---|---|---|---|---|---|
| `version` | 0.7.0 | 0.7.0 | 0.7.0 | 0.8.0 | **0.8.0** | Real conflict: pdfcraft path deps say `version = "0.8.0"`. Tested with 0.7.0: `failed to select a version for the requirement pdfcraft-content = "^0.8.0"`. Word/grid/deck crates become 0.8.0 (or must switch to per-crate versions) |
| `rust-version` | 1.90 | 1.90 | 1.95 | 1.95 | 1.95 | word/grid MSRV raised; harmless on 1.97.1 |
| `edition` | 2024 | 2024 | 2024 | 2024 | 2024 | same |
| `license` | MIT OR Apache-2.0 | same | same | same | same | same |
| `repository` | storytold/wordcraft | .../gridcraft | .../deckcraft | .../pdfcraft | one URL | only 12 pdfcraft crates inherit it |
| `resolver` | 3 | 3 | 3 | 3 | 3 | same |

### 3.2 `[workspace.lints]` (all member crates inherit it via `[lints] workspace = true`, so one set for all)

Common: `rust.unsafe_code = "forbid"`, `clippy.dbg_macro/todo = "warn"`. wordcraft + gridcraft add `too_many_arguments`, `type_complexity` = allow;
deckcraft adds those plus `self_named_constructors`, `large_enum_variant`, `result_large_err` = allow; pdfcraft allows none. The spike uses the union,
so pdfcraft's crates lose those clippy warnings (`cargo clippy` only). Per-app lint sets would need per-crate `[lints.clippy]` edits.

### 3.3 `[profile.*]` (root only; one set for the whole workspace)

| | wordcraft / gridcraft / deckcraft | pdfcraft |
|---|---|---|
| `dev` | opt-level 1 | opt-level 1 |
| `dev.package."*"` | opt-level 2 | opt-level 2, `debug = "line-tables-only"` (cuts 500-600 MB test PDBs) |
| `release` | opt 3, thin LTO, codegen-units 16, debug 0 | thin LTO, **codegen-units 1** |
| other | - | `profile.low-disk` |

Spike took the 16-codegen-units release profile plus pdfcraft's dev settings and `low-disk`. codegen-units 1 for all would slow every app's
release build; 16 changes pdfcraft's output/size slightly. `[profile.release.package.pdfcraft-*]` can restore the old value.

### 3.4 `[patch]` (workspace root only: a merged workspace gets exactly one set, applied to all apps)

| Patch | Where today | Merged consequence |
|---|---|---|
| hayro, hayro-syntax, hayro-interpret, hayro-jbig2 | pdfcraft `vendor/` AND craft-libs `vendor/` (two **different** forks: src differs - hayro 2 files, syntax 15, interpret 10; jbig2 identical) | Only one can win. Spike picked pdfcraft's. wordcraft/deckcraft (crates.io hayro-syntax) now silently use the fork |
| lopdf, winit, egui-wgpu, vello_common | pdfcraft `vendor/` only | Now applied to wordcraft/gridcraft/deckcraft too (patched winit and egui-wgpu for every app; vello_common patch hits deckcraft's 0.2) |
| krilla | deckcraft: git craft-libs rev c348732; craft-libs: its `vendor/krilla`; wordcraft: unpatched crates.io 0.8 | One path patch (craft-libs vendor). wordcraft's krilla switches to the fork; feature `rayon` now on for krilla users |
| craft-ui, craft-print, craft-quit, craft-pdf (git deps) | word/grid: git rev d27646a; pdfcraft: 3 other revs; deckcraft: c348732 | Baseline lock has 2 craft-ui copies, 2 craft-print copies, 1 craft-quit, 1 craft-pdf. Variant `Cargo.toml.craftlibs-patched` with `[patch."https://github.com/storytold/craft-libs"]` pointing to a local checkout resolved fine and leaves a single copy of each |

Cargo reported no unused patch.

### 3.5 Crate name collisions

Of 110 manifests (crates, apps, xtask) the only identical package name is `xtask` (x4, one per app; skipped in the spike - rename to `xtask-<app>` or merge).
All other crates are prefixed per app (`wordcraft-fonts`, `deckcraft-fonts`, `pdfcraft-fonts`, ...). craft-libs' `craft-pdf` / `craft-print` do not clash with the apps' `*-pdf` / `*-print`.
Binary target names were not checked.

### 3.6 Features on shared crates (merged `cargo metadata`, workspace-wide union)

Caveat: `cargo metadata` shows the union over all members. Building one app with `-p <app>` under resolver 3 unifies only that package's graph, so most
of these gains do not occur in per-app builds; they appear in `--workspace` builds. Gains versus each app alone: eframe `glow, persistence, ron, serde`; egui-winit
`serde, links, webbrowser`; image `gif, webp` (gridcraft) / `webp` (pdfcraft); arboard `wayland-data-control`; rfd `file-handle-inner`; kurbo `serde`; sha1/sha2 `default, oid, alloc`
(gridcraft); krilla `rayon, pdf`; hayro-syntax `default, images, unsafe` (deckcraft); getrandom 0.3/0.4 `wasm_js`; serde_json `alloc`; web-sys ~25 more interfaces
(Audio*, Idb*, Request/Response...). No crate ends up with a feature outside the union of the apps' own features.

## 4. Duplicated-role crates

Measured over `src/**/*.rs`: files / lines / distinct `fn` names. Overlap = Jaccard of fn names.

| Role | wordcraft | gridcraft | deckcraft | pdfcraft | Overlap | Verdict |
|---|---|---|---|---|---|---|
| geom | 2 / 605 / 49 | - | 7 / 5,576 / 201 | 2 / 242 / 18 | 0.01-0.03 | Same name, unrelated code. Not dedupable as is |
| fonts | 9 / 4,974 / 251 | - | 5 / 2,635 / 144 | 11 / 4,863 / 219 | word~deck 0.23 (73 shared fns); pdf~others 0.04 | word and deck share ancestry (`fontdb.rs`, `script.rs`, `lib.rs` all diverged: 1,696 / 339 / 615 differing lines). pdfcraft-fonts is a different design (std14, embedding, encodings). Partial, costly merge |
| metafile | 9 / 3,212 / 214 | - | 13 / 7,043 / 399 | - | **0.48** (200 shared fns, 22 shared types) | deckcraft's is documented as "ported from WordCraft". `bytes.rs` and `geom.rs` byte-identical; `canvas/dib/emf/wmf/place/lib/tests` diverged (56-1,040 differing lines each); deck adds `clip, cliprect, emfplus, text`. Best dedup candidate; deck is the superset |
| pdf (writer) | 2 / 3,091 / 115 | 3 / 1,126 / 61 | 2 / 1,096 / 43 | craft-libs `pdf`: 4 / 481 / 24 | 0.00-0.03 | Independent code; deckcraft already uses `craft-pdf` from craft-libs. No reuse between them |
| render | 5 / 3,006 / 107 | - | 25 / 13,071 / 495 | 11 / 13,988 / 565 | 0.02 | Unrelated implementations |
| mcp | 5 / 1,068 / 54 | 6 / 1,636 / 77 | 6 / 1,463 / 68 | - | fn 0.25-0.33; type names 0.56-0.83 (5 shared) | Same skeleton: good candidate for a shared `craft-mcp` core |
| ui-egui | 67 / 53.8k | 40 / 25.6k | 62 / 30.9k | 84 / 58.7k | fn 0.04-0.08 (100-180 shared names) | Shared names are generic (new/default/ui); already partly extracted to craft-ui |
| engine | 70 / 50.6k | 37 / 31.2k | 60 / 33.5k | 29 / 19.6k | 0.02-0.07 | Separate per-document engines |
| print | - | - | - | 4 / 1,825 / 67 | 0.07 vs craft-libs `print` (8 / 3,552 / 150) | pdfcraft keeps composition; spooler already in craft-libs |

## 5. Duplicate dependency summary (`cargo tree -d --workspace -e normal`)

| Scope | Crate names with 2+ versions | Version entries listed |
|---|---|---|
| wordcraft alone (locked) | 15 | 34 |
| gridcraft alone | 7 | 15 |
| deckcraft alone | 17 | 38 |
| pdfcraft alone | 27 | 58 |
| **Merged workspace (final lock)** | **32** | **67** |

(The first resolve, before re-running `cargo generate-lockfile`, showed 34 names / 77 entries. The merged lock is a fresh latest-compatible
resolve, not seeded from the upstream lockfiles; seed it to keep reproducibility.)

Worst offenders (merged): read-fonts x4 (0.39, 0.41, 0.43, 0.44), skrifa x3 (0.42, 0.44, 0.47), getrandom x3, hashbrown x3, syn x3, vello_common x3 (0.0.8, 0.1, 0.2),
vello_cpu x3, harfrust x2 (0.12, 0.13), kurbo x2 (0.11, 0.13), tiny-skia-path x2, png x2, windows / windows-core / windows-result / windows-sys x2,
phf* x2, base64 x2, pollster x2, weezl x2, imagesize x2, miniz_oxide x2, rustc-hash x2, bitflags x2, font-types x2, craft-ui x2 and craft-print x2 (git revs).

New in the merged tree versus any single app: craft-ui, craft-print (different craft-libs revs; gone with the `[patch]` variant, 34 -> 32 names measured on the first lock),
imagesize (0.13 pdfcraft vs 0.14 word/deck), tiny-skia-path (0.11 pdfcraft vs 0.12 word/deck).

Avoidable by aligning requirement strings only: harfrust (pdfcraft inline 0.12 -> 0.13), skrifa 0.44 (pdfcraft inline), pollster, getrandom 0.3 -> 0.4.
Tied to pdfcraft's vendored hayro/vello forks (harder): vello_common 0.0.8/0.1, kurbo 0.11, tiny-skia-path 0.11.

## 6. Required upstream edits

- **To resolve in a merged workspace: none.** No upstream file was edited; every conflict was handled in the new root `Cargo.toml`.
- **To adopt it for real:**
  1. Delete the four app root manifests and lockfiles (members cannot sit under another `[workspace]`), moving `[workspace.*]`, `[patch]`, `[profile]` into one root.
  2. Decide a version policy: 0.8.0 for all, or per-crate `version` instead of `version.workspace = true` (pdfcraft's intra-workspace `version = "0.8.0"` requirements force this).
  3. Rename or merge the four `xtask` packages.
  4. Reconcile the two vendored forks of hayro / hayro-syntax / hayro-interpret (pdfcraft vs craft-libs) and decide whether wordcraft/deckcraft accept pdfcraft's patched winit, egui-wgpu, vello_common, lopdf.
  5. Optional dedupe edits: pdfcraft inline `harfrust 0.12` / `skrifa 0.44` / `pollster 1`; gridcraft `skrifa` (spike forces 0.47, compile unverified).
  6. Not checked: relative `include_bytes!`/asset paths (the copy kept the repo layout), build scripts, wasm targets, and anything that needs actual compilation.
- **Environment note:** cargo's built-in git fetch fails on this machine with `CRYPT_E_NO_REVOCATION_CHECK`; use `CARGO_NET_GIT_FETCH_WITH_CLI=true` and `CARGO_HTTP_CHECK_REVOKE=false`.
