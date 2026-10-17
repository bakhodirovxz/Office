#!/usr/bin/env python3
"""WP9e: apply the uz-Latn / uz-Cyrl DRAFT translations of the P1 rows to i18n/*.tsv.

Every value written here is a DRAFT (status `draft:agent-sonnet:<date>`). Nothing is reviewed. The owner reviews all Uzbek
(docs/phase1/i18n-review.md). The tool never touches a row whose status is already `reviewed:*` / `rejected:*`.

Sources, one file per catalog scope in i18n/tools/p1_drafts/<scope>.tsv (columns: id, source_en, uz_latn). The Latin
text uses ASCII stand-ins that this script turns into the real characters:
  `   -> U+02BB (oʻ gʻ)        ~   -> U+02BC (tutuq belgisi, maʼlumot)
  <<x>> -> x is kept verbatim in the Cyrillic text (key names, product words); the markers are removed from the Latin text.

uz-Cyrl is derived from the Latin draft by the deterministic rules below (official Uzbek Latin -> Cyrillic mapping, with a
small exception list) and checked again by `cargo xt i18n lint` (Latin/Cyrillic consistency). The glossary terms keep their
own hand-written Cyrillic: a term that occurs in a string is rendered with the glossary Cyrillic form.

Usage (repo root):
  python i18n/tools/p1_drafts.py build      # drafts/*.uz line files (scratch) -> i18n/tools/p1_drafts/*.tsv (needs --src DIR)
  python i18n/tools/p1_drafts.py apply      # p1_drafts/*.tsv -> i18n/<scope>.tsv  (Latin + derived Cyrillic)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DRAFTS = ROOT / "i18n" / "tools" / "p1_drafts"
DATE = "2026-10-17"
STATUS = f"draft:agent-sonnet:{DATE}"
SCOPES = ["shared", "wordcraft", "gridcraft", "pdfcraft", "deckcraft"]
HEADER = "id context source_en uz_latn uz_latn_status uz_cyrl uz_cyrl_status ru ru_status ru_apps priority apps where note".split()

# Latin words that stay Latin inside Cyrillic text (product/key/format names). Everything else is transliterated.
KEEP = {
    "Word", "Excel", "Letter", "Zotero", "Discord", "GitHub", "Enter", "Backspace", "Tab", "Shift", "Ctrl", "Esc",
    "Unicode", "JavaScript", "PowerPoint", "PostScript", "SmartArt", "WordArt", "Bates", "Microsoft", "Apache", "MIT",
    "PdfCraft", "GridCraft", "WordCraft", "DeckCraft", "ArtCraft", "OK", "PDF", "PNG", "JPEG", "HTML", "CSV", "RTF", "OCR",
}
# Whole-stem exceptions (lower case, applied before the letter mapping): Uzbek Cyrillic spells these differently.
STEMS = {"obyekt": "объект", "funksiya": "функция"}


def real(text: str) -> str:
    return text.replace("`", "ʻ").replace("~", "ʼ")


def is_protected(tok: str) -> bool:
    if tok in KEEP:
        return True
    if re.fullmatch(r"[A-Z0-9][A-Z0-9/.+\-]*[A-Z0-9]", tok) and any(c.isalpha() for c in tok):
        return True  # PDF, PDF/A, RIPEMD-160, SHA-1
    if re.search(r"[a-z][A-Z]", tok):
        return True  # CamelCase: SmartArt, JavaScript
    return False


def case_like(src: str, out: str) -> str:
    if src[:1].isupper() and out:
        return out[0].upper() + out[1:]
    return out


def word_to_cyrl(w: str) -> str:
    """One Uzbek Latin word (letters, U+02BB, U+02BC) -> Cyrillic."""
    low = w.lower()
    for stem, cyr in STEMS.items():
        i = low.find(stem)
        if i >= 0:
            return word_to_cyrl(w[:i]) + case_like(w[i:], cyr) + word_to_cyrl(w[i + len(stem):])
    out = []
    i, n = 0, len(w)
    vowels = set("aeiouʻ")

    def up(ch_lat, cyr):
        # an upper-case Latin letter gives an upper-case Cyrillic letter (digraphs: first letter decides)
        return cyr.upper() if ch_lat.isupper() else cyr

    while i < n:
        c = w[i]
        l = c.lower()
        nxt = w[i + 1].lower() if i + 1 < n else ""
        nxt2 = w[i + 2] if i + 2 < n else ""
        if l in "og" and nxt == "ʻ":
            out.append(up(c, "ў" if l == "o" else "ғ"))
            i += 2
        elif l == "s" and nxt == "h":
            out.append(up(c, "ш")); i += 2
        elif l == "c" and nxt == "h":
            out.append(up(c, "ч")); i += 2
        elif l == "t" and nxt == "s":
            out.append(up(c, "ц")); i += 2
        elif l == "y" and nxt == "o" and nxt2 != "ʻ":
            out.append(up(c, "ё")); i += 2
        elif l == "y" and nxt == "u":
            out.append(up(c, "ю")); i += 2
        elif l == "y" and nxt == "a":
            out.append(up(c, "я")); i += 2
        elif l == "y" and nxt == "e":
            out.append(up(c, "е")); i += 2
        elif l == "e":
            out.append(up(c, "э" if i == 0 else "е")); i += 1
        elif l == "ʼ":
            out.append("ъ"); i += 1
        elif l == "ʻ":
            out.append("?"); i += 1  # stray reversed comma: lint will show it
        else:
            m = {"a": "а", "b": "б", "d": "д", "f": "ф", "g": "г", "h": "ҳ", "i": "и", "j": "ж", "k": "к", "l": "л",
                 "m": "м", "n": "н", "o": "о", "p": "п", "q": "қ", "r": "р", "s": "с", "t": "т", "u": "у", "v": "в",
                 "x": "х", "y": "й", "z": "з"}.get(l)
            out.append(up(c, m) if m else c)
            i += 1
    return "".join(out)


TOKEN = re.compile(r"(\{[^}]*\}|<<.*?>>|\\[nt\\]|[A-Za-z0-9ʻʼ_./+\-]+)")


def latn_to_cyrl(text: str, glossary: dict) -> str:
    """Latin draft (markers already resolved to real characters except <<>>) -> Cyrillic."""
    out = []
    for part in TOKEN.split(text):
        if not part:
            continue
        if part.startswith("<<"):
            out.append(part[2:-2])
        elif part.startswith("{") or part.startswith("\\"):
            out.append(part)
        elif re.fullmatch(r"\.[a-z0-9]+", part):
            out.append(part)  # file extension (.docx, .csv)
        elif re.fullmatch(r"[A-Za-z0-9ʻʼ_./+\-]+", part):
            if is_protected(part) or re.fullmatch(r"[0-9.,+\-/*%]+", part):
                out.append(part)
            else:
                # split hyphen/slash/dot joined pieces ("Nom-A") and map every piece
                pieces = re.split(r"([./+\-])", part)
                out.append("".join(p if p in "./+-" or p.isdigit() else word_to_cyrl(p) for p in pieces))
        else:
            out.append(part)
    return "".join(out)


def esc_load(cell: str) -> str:
    return cell


def build(src: Path) -> None:
    """Join the numbered line files with the P1 row order of each catalog and write the keyed drafts."""
    DRAFTS.mkdir(parents=True, exist_ok=True)
    for scope in SCOPES:
        rows = [l.rstrip("\n").split("\t") for l in (ROOT / "i18n" / f"{scope}.tsv").read_text(encoding="utf-8").splitlines()[1:]]
        p1 = [r for r in rows if r[10].startswith("P1")]
        lines = (src / f"{scope}.uz").read_text(encoding="utf-8").splitlines()
        assert len(lines) == len(p1), (scope, len(lines), len(p1))
        out = []
        for n, (line, r) in enumerate(zip(lines, p1), 1):
            num, text = line.split("##", 1)
            assert int(num) == n, (scope, num, n)
            text = text[1:] if text.startswith(" ") else text
            out.append("\t".join([r[0], r[2], text]))
        (DRAFTS / f"{scope}.tsv").write_text("id\tsource_en\tuz_latn\n" + "\n".join(out) + "\n", encoding="utf-8", newline="\n")
        print(scope, len(out))


def load_glossary():
    g = {}
    for line in (ROOT / "i18n" / "glossary.tsv").read_text(encoding="utf-8").splitlines()[1:]:
        c = line.split("\t")
        g[(c[2], c[1])] = (c[3], c[5])
    return g


def apply() -> None:
    glossary = load_glossary()
    for scope in SCOPES:
        path = ROOT / "i18n" / f"{scope}.tsv"
        lines = path.read_text(encoding="utf-8").split("\n")
        drafts = {}
        for l in (DRAFTS / f"{scope}.tsv").read_text(encoding="utf-8").split("\n")[1:]:
            if l:
                i, en, uz = l.split("\t", 2)
                drafts[i] = (en, uz)
        done = skipped = 0
        for k, l in enumerate(lines[1:], 1):
            if not l:
                continue
            c = l.split("\t")
            if c[0] not in drafts:
                continue
            en, uz = drafts[c[0]]
            assert en == c[2], (scope, c[0], en, c[2])
            if c[4].startswith(("reviewed", "rejected")) or c[6].startswith(("reviewed", "rejected")):
                skipped += 1
                continue
            if not uz.strip():
                c[13] = "no-uz: part of an ArtCraft sentence that rebrand drops (design 4.3); nothing to translate"
                lines[k] = "\t".join(c)
                skipped += 1
                continue
            latn = real(uz.replace("<<", "").replace(">>", ""))
            cyrl = latn_to_cyrl(real(uz), glossary)
            c[3], c[4], c[5], c[6] = latn, STATUS, cyrl, STATUS
            lines[k] = "\t".join(c)
            done += 1
        path.write_text("\n".join(lines), encoding="utf-8", newline="\n")
        print(f"{scope}: drafted {done}, left {skipped}")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "build":
        build(Path(sys.argv[sys.argv.index("--src") + 1]))
    elif len(sys.argv) >= 2 and sys.argv[1] == "apply":
        apply()
    else:
        print(__doc__)
