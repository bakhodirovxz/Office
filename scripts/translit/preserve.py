#!/usr/bin/env python3
"""Phase 2 WP9: compare B (open -> save) with R (open -> command -> save) for .xlsx and .pptx (PLAN 8.2).

  preserve.py <xlsx|pptx> B R      exit 0 and print IDENTICAL, or print every delta and exit 1

xlsx: every part except xl/sharedStrings.xml is byte-identical; in sharedStrings only the text of <t> may change.
pptx: every part except ppt/slides/slide*.xml and ppt/notesSlides/*.xml is byte-identical; in those the trees are equal,
ignoring the a:t text of a:r (an a:r that lost its text, the tail of a digraph, may vanish); a:fld is compared with its a:t.
"""
import re, sys, zipfile
import xml.etree.ElementTree as ET


def local(tag):
    return tag.rsplit('}', 1)[-1]


def is_text_part(kind, name):
    if kind == 'xlsx':
        return name == 'xl/sharedStrings.xml' or bool(re.fullmatch(r'xl/worksheets/sheet\d+\.xml', name))
    return bool(re.fullmatch(r'ppt/(slides/slide\d+|notesSlides/notesSlide\d+)\.xml', name))


def text_of(n, run):
    return ''.join((c.text or '') for c in n if local(c.tag) == 't')


def diff(b, r, path, kind, out):
    if b.tag != r.tag or b.attrib != r.attrib:
        out.append(f'{path}: element differs: {local(b.tag)} {b.attrib} vs {local(r.tag)} {r.attrib}')
        return
    # character content of <t> is the only thing allowed to change (a:t inside a:fld must be equal)
    in_fld = path.endswith('/fld')
    if local(b.tag) == 't' and in_fld is False:
        pass
    elif (b.text or '').strip() != (r.text or '').strip():
        out.append(f'{path}: character content differs: {b.text!r} vs {r.text!r}')
    bk, rk = list(b), list(r)
    i = j = 0
    while i < len(bk) or j < len(rk):
        here = f'{path}/{local(bk[i].tag) if i < len(bk) else "?"}[{i}]'
        if i < len(bk) and j < len(rk):
            sub = []
            diff(bk[i], rk[j], path + '/' + local(bk[i].tag), kind, sub)
            if not sub:
                i += 1; j += 1
            elif local(bk[i].tag) in ('r',) and len(text_of(bk[i], True)) <= 2:
                i += 1  # digraph tail run vanished
            else:
                out.extend(sub); i += 1; j += 1
        elif i < len(bk):
            if not (local(bk[i].tag) == 'r' and len(text_of(bk[i], True)) <= 2):
                out.append(f'{here}: missing from R')
            i += 1
        else:
            out.append(f'{path}: extra {local(rk[j].tag)} in R'); j += 1


def fix_fld_text(n, in_fld=False):
    # inside a:fld the a:t must match: mark it by renaming so that diff() compares its text
    for c in n:
        if local(n.tag) == 'fld' and local(c.tag) == 't':
            c.tag = c.tag.replace('}t', '}fldt')
        fix_fld_text(c)


def sig(n):
    return (n.tag, tuple(sorted(n.attrib.items())), tuple(sig(c) for c in n))


def merge_runs(n):
    """pptx: adjacent a:r with identical a:rPr become one (the engine's normalize() merges them in R but not in B)."""
    kids = list(n)
    for c in kids:
        merge_runs(c)
    prev = None
    for c in kids:
        if local(c.tag) == 'r' and prev is not None and local(prev.tag) == 'r':
            rp = [k for k in c if local(k.tag) == 'rPr']; pp = [k for k in prev if local(k.tag) == 'rPr']
            if [sig(k) for k in rp] == [sig(k) for k in pp]:
                tp = [k for k in prev if local(k.tag) == 't'][0]; tc = [k for k in c if local(k.tag) == 't'][0]
                tp.text = (tp.text or '') + (tc.text or '')
                n.remove(c)
                continue
        prev = c


def drop_cached_strings(n):
    """xlsx: the cached text result of a formula cell (t="str" with <f>) follows the cells it reads, so it may change."""
    for c in n.iter():
        if local(c.tag) == 'c' and c.get('t') == 'str' and any(local(k.tag) == 'f' for k in c):
            for k in c:
                if local(k.tag) == 'v':
                    k.text = ''


def main():
    kind, fb, fr = sys.argv[1:4]
    zb, zr = zipfile.ZipFile(fb), zipfile.ZipFile(fr)
    nb, nr = sorted(zb.namelist()), sorted(zr.namelist())
    errs = []
    if nb != nr:
        errs.append(f'package parts differ: only in B {sorted(set(nb) - set(nr))}, only in R {sorted(set(nr) - set(nb))}')
    for n in nb:
        if n not in nr or n.endswith('/'):
            continue
        a, c = zb.read(n), zr.read(n)
        if is_text_part(kind, n):
            tb, tr = ET.fromstring(a), ET.fromstring(c)
            if kind == 'pptx':
                merge_runs(tb); merge_runs(tr)
            if kind == 'xlsx' and n.startswith('xl/worksheets/'):
                drop_cached_strings(tb); drop_cached_strings(tr)
            fix_fld_text(tb); fix_fld_text(tr)
            d = []
            diff(tb, tr, n, kind, d)
            errs.extend(d)
        elif a != c:
            errs.append(f'{n} is not byte-identical ({len(a)} vs {len(c)} bytes)')
    if errs:
        print('\n'.join(errs)); sys.exit(1)
    print('IDENTICAL')


main()
