# vendor-notes: owner-provided reference material

Copied read-only from the HRM **test** server `vds-test` (<test-server>) on 2026-10-19, at the owner's request.

| Path | Source on the server | Used in |
|---|---|---|
| `uz-translit-onlyoffice/` | `<server-path>` (mounted into the `onlyoffice-docs` container as plugin `asc.{CFA126E3-AA8A-4457-B4B7-99AA619AF960}`) | Phase 2: seed rules for the `uz-translit` crate |
| `merge_lines.py` | `/tmp/claude-1000/-srv/214d3298-…/scratchpad/merge_lines.py` (sha256 prefix `9563dd3ae3ff705d`) | Phase 3: PDF→DOCX sidecar post-processor |

**Licensing and clean-room**
- Both are the owner's own code. The plugin's `config.json` says `"offered": "Uzbekgidroenergo HRM"`, and there are no
  ONLYOFFICE copyright headers.
- The plugin talks to the ONLYOFFICE plugin API. That makes it reference material only: Phase 2 re-implements the rules in
  Rust and copies nothing.
- `merge_lines.py` imports `python-docx`. It runs inside the PDF→DOCX sidecar container next to pdf2docx (GPL), and is
  never linked into our code.
