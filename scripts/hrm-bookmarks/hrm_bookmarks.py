#!/usr/bin/env python3
"""Phase 5 WP9 (synthetic half): `_hrm_*` bookmark round trip through the Docs CLI. Invented content only, no corpus.

  python -I scripts/hrm-bookmarks/hrm_bookmarks.py <docs-cli> [--keep DIR]     generate, round-trip, check; exit 1 on any loss
  python -I scripts/hrm-bookmarks/hrm_bookmarks.py --gen DIR                   only write the fixtures
  python -I scripts/hrm-bookmarks/hrm_bookmarks.py --check ORIG SAVED [B1|T1]  compare one saved file with its original (the Playwright round trip)

Names are the ones HRB syncs (docs/phase0/hr-backend-prompt.md:109-111). HRB uses them in the body only (hrm-current-state.md:153),
so no header/footer bookmarks. Per fixture: B1 = open -> edit text at the end -> save (F13/R4 test); T1 = open -> translitCyrl -> save;
T2 = T1 -> translitLatn -> save. A bookmark must keep its name, its zero-length/non-empty state and its paragraph anchors, and its
range text must be unchanged (B1, T2) or Cyrillic (T1).
"""
import os, re, subprocess, sys, tempfile, zipfile
import xml.etree.ElementTree as ET

W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
q = lambda t: f'{{{W}}}{t}'
_id = [0]


def r(text, bold=False):
    return f'<w:r>{"<w:rPr><w:b/></w:rPr>" if bold else ""}<w:t xml:space="preserve">{text}</w:t></w:r>'


def bs(name):
    _id[0] += 1
    return f'<w:bookmarkStart w:id="{_id[0]}" w:name="{name}"/>', f'<w:bookmarkEnd w:id="{_id[0]}"/>'


def p(*parts):
    return f'<w:p>{"".join(parts)}</w:p>'


def tbl(*rows):  # rows of cells, each cell a list of paragraph xml
    body = ''.join('<w:tr>' + ''.join(f'<w:tc><w:tcPr><w:tcW w:w="4500" w:type="dxa"/></w:tcPr>{"".join(c)}</w:tc>' for c in row) + '</w:tr>' for row in rows)
    return f'<w:tbl><w:tblPr><w:tblW w:w="9000" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr>{body}</w:tbl>'


def decree():
    a, b = bs('_hrm_desc_'); s, e = bs('_hrm_sig_')
    return [p(r('Buyruq raqami shabon', True)),
            p(a, r('Xodimlarni ish bilan taminlash '), r('haqida', True), r(' qaror qabul qilindi.')),  # spans 3 runs
            p(r('Ikkinchi band mazmuni va asoslari.'), b),                                             # range ends in a later paragraph
            p(r('Imzo joyi')), p(s, e),                                                                # zero-length, bookmark-only paragraph
            p(r('Tasdiqlandi.'))]


def letter():
    a, b = bs('_hrm_desc_'); c, d = bs('_hrm_plan_'); s, e = bs('_hrm_sig_'); g, h = bs('_hrm_agree_')
    return [p(r('Xat mavzusi', True)),
            tbl([[p(r('Mazmun:'))], [p(a, r('Xat matni bu yerda '), r('yozilgan', True), b)]],      # inside a table cell
                [[p(r('Reja:'))], [p(c, r('Birinchi qadam, '), r('ikkinchi qadam'), d)]]),           # spans runs inside a table
            p(r('Kelishildi: '), s, e, r(' imzo'), g, r('Kelishuv bandi matni'), h),                    # zero-length mid-text + agree
            p(r('Tasdiqlandi.'))]


def report():
    a, b = bs('_hrm_rsmry_'); c, d = bs('_hrm_rtask_'); s, e = bs('_hrm_rcont_')
    return [p(r('Hisobot', True)),
            p(a, r('Qisqacha mazmun '), r('shu yerda', True), b),
            tbl([[p(r('Topshiriq')), p(c, r('Topshiriq tavsifi '), r('birinchi'), d)], [p(r('Davomi')), p(r('ikkinchi katak'))]]),
            p(s, e), p(r('Kontent bandi boshlandi')),                                                  # zero-length bookmark-only paragraph
            p(r('Tasdiqlandi.'))]


FIXTURES = {'decree': decree, 'letter': letter, 'report': report}
CT = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
RELS = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'


def write(path, paras):
    doc = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{"".join(paras)}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>'
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr('[Content_Types].xml', CT); z.writestr('_rels/.rels', RELS); z.writestr('word/document.xml', doc)


def gen(d):
    os.makedirs(d, exist_ok=True)
    for n, f in FIXTURES.items():
        _id[0] = 0; write(os.path.join(d, n + '.docx'), f())


def anchors(path):
    """name -> (text in range, paragraph index of start, of end, text before start); body order, p end = newline."""
    root = ET.fromstring(zipfile.ZipFile(path).read('word/document.xml'))
    out, open_, buf, pi = {}, {}, [], [0]

    def walk(n):
        t = n.tag
        if t == q('bookmarkStart'): open_[n.get(q('id'))] = (n.get(q('name')), len(buf), pi[0])
        elif t == q('bookmarkEnd'):
            nm, i, ps = open_.pop(n.get(q('id')))
            out[nm] = (''.join(buf[i:]), ps, pi[0], ''.join(buf[:i])[-10:])
        elif t == q('t'): buf.append(n.text or '')
        for c in n: walk(c)
        if t == q('p'): buf.append('\n'); pi[0] += 1
    walk(root)
    return out


def cli(exe, src, dst, *cmds):
    a = [exe, 'run', '--file', src] + [x for c in cmds for x in ('--cmd', c)] + ['--save', dst]
    res = subprocess.run(a, capture_output=True, text=True)
    return res.returncode == 0 and os.path.exists(dst), res.stderr.strip()[-300:]


CYR = re.compile('[Ѐ-ӿ]')


def verdict(key, want, got):
    """'survived' or the reason not; want/got are anchors() entries. key: B1/T2 = text unchanged, T1 = Cyrillic."""
    txt, ps, pe, ctx = want
    if got is None: return 'LOST'
    if (got[1], got[2]) != (ps, pe) or bool(got[0].strip('\n')) != bool(txt.strip('\n')): return f'MOVED paras {got[1]}-{got[2]} vs {ps}-{pe}, text {got[0]!r} vs {txt!r}'
    if key == 'T1': return 'survived' if not txt.strip() or CYR.search(got[0]) else f'MOVED not Cyrillic {got[0]!r}'
    if got[0] != txt or (key == 'B1' and got[3] != ctx): return f'MOVED text {got[0]!r} vs {txt!r}; before {got[3]!r} vs {ctx!r}'
    return 'survived'


def check(orig, saved, key):
    """--check mode (web round trip): every bookmark of ORIG must survive in SAVED; exit 1 on a loss."""
    base, now = anchors(orig), anchors(saved)
    xml = zipfile.ZipFile(saved).read('word/document.xml').decode()
    bad = 0 if ('Qoshimcha' in xml if key == 'B1' else CYR.search(xml)) else 1   # the edit / translit really happened
    if bad: print(f'FAIL {key}: the saved file does not show the edit')
    for name in base:
        v = verdict(key, base[name], now.get(name))
        bad += v != 'survived'
        print(f'{"ok  " if v == "survived" else "FAIL"} {name:13} {key}: {v}')
    print('RESULT', 'all bookmarks survived' if not bad else f'{bad} problem(s)')
    return 1 if bad else 0


def main():
    argv = sys.argv[1:]
    if argv[:1] == ['--gen']: gen(argv[1]); return 0
    if argv[:1] == ['--check']: return check(argv[1], argv[2], argv[3] if len(argv) > 3 else 'B1')
    exe = argv[0]; keep = argv[argv.index('--keep') + 1] if '--keep' in argv else None
    tmp = keep or tempfile.mkdtemp(); gen(tmp); bad = 0
    for n in FIXTURES:
        src = f'{tmp}/{n}.docx'; base = anchors(src)
        steps = {}
        for key, cmds in (('B1', ['caret.docEnd={}', 'text.insert={"text":" Qoshimcha matn."}']), ('T1', ['tools.translitCyrl={}'])):
            ok, err = cli(exe, src, f'{tmp}/{n}.{key}.docx', *cmds)
            if not ok: print(f'FAIL {n} {key}: CLI error {err}'); bad += 1; continue
            steps[key] = anchors(f'{tmp}/{n}.{key}.docx')
            if key == 'B1' and 'Qoshimcha' not in zipfile.ZipFile(f'{tmp}/{n}.B1.docx').read('word/document.xml').decode(): print(f'FAIL {n} B1: edit not applied'); bad += 1
        if 'T1' in steps:
            ok, err = cli(exe, f'{tmp}/{n}.T1.docx', f'{tmp}/{n}.T2.docx', 'tools.translitLatn={}')
            if ok: steps['T2'] = anchors(f'{tmp}/{n}.T2.docx')
            else: print(f'FAIL {n} T2: CLI error {err}'); bad += 1
        for name in base:
            for key in ('B1', 'T1', 'T2'):
                v = verdict(key, base[name], steps.get(key, {}).get(name))
                if v != 'survived': bad += 1
                print(f'{"ok  " if v == "survived" else "FAIL"} {n:7} {name:13} {key}: {v} (len {len(base[name][0])}, zero-length={not base[name][0]})')
    print('RESULT', 'all bookmarks survived' if not bad else f'{bad} problem(s)')
    return 1 if bad else 0


if __name__ == '__main__': sys.exit(main())
