// node --test ci/npm : the built package (PKG_DIR, default $OFFICE_TARGET/dist/npm/uz-translit) against the Rust golden lists.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const pkg = path.resolve(process.env.PKG_DIR ?? path.join(process.env.OFFICE_TARGET ?? 'E:/office-target/p1', 'dist/npm/uz-translit'));
const m = await import(pathToFileURL(path.join(pkg, 'uz_translit.js')).href);
m.initSync({ module: readFileSync(path.join(pkg, 'uz_translit_bg.wasm')) });
const { toCyrillic, toLatin, convert, convertRuns, detect } = m;

// Same reading of the tsv as crates/uz-translit/tests/golden.rs: reviewed rows only, default options.
function table(name) {
  const lines = readFileSync(path.join(root, 'crates/uz-translit/data', name), 'utf8').split(/\r?\n/).filter((l) => l && !l.startsWith('#'));
  const head = lines.shift().split('\t');
  return lines.map((l) => Object.fromEntries(head.map((h, i) => [h, l.split('\t')[i] ?? ''])));
}

test('golden.tsv parity with the Rust crate', () => {
  const fails = [];
  let fwd = 0, back = 0;
  for (const r of table('golden.tsv').filter((r) => r.status.startsWith('reviewed:'))) {
    if (['both', 'l2c'].includes(r.dir)) { fwd++; const o = toCyrillic(r.latin); if (o !== r.cyrillic) fails.push(`${r.id} l2c ${o}`); }
    if (['both', 'c2l'].includes(r.dir)) { back++; const o = toLatin(r.cyrillic); if (o !== r.latin) fails.push(`${r.id} c2l ${o}`); }
  }
  assert.deepEqual(fails, []);
  assert.ok(fwd >= 500 && back >= 500, `rows checked: ${fwd}/${back}`);
});

test('variants.tsv: apostrophe variants all give the same Cyrillic', () => {
  const rows = table('variants.tsv').filter((r) => r.status.startsWith('reviewed:'));
  assert.ok(rows.length > 50);
  for (const r of rows) assert.equal(toCyrillic(r.input), r.cyrillic, r.id);
});

test('convert(), direction and apostrophe option', () => {
  assert.equal(convert('salom', 'cyrl'), 'салом');
  assert.equal(convert('салом', 'latn'), 'salom');
  assert.equal(toLatin('Ўзбекистон'), 'Oʻzbekiston');
  assert.equal(toLatin('Ўзбекистон', { apostrophe: 'ascii' }), "O'zbekiston");
  for (const v of ["O'zbek", 'O‘zbek', 'O’zbek', 'Oʻzbek', 'O`zbek']) assert.equal(toCyrillic(v), 'Ўзбек', v);
});

test('2026 alphabet (cases from tests/alphabet2026.rs)', () => {
  const o = { latinAlphabet: '2026' };
  assert.equal(toLatin('шаҳар чой ўзбек ғалаба', o), 'şahar çoy özbek ğalaba');
  assert.equal(toLatin('Исҳоқ асҳоб ИСҲОҚ', o), 'Ishoq ashob ISHOQ');
  assert.equal(toLatin('Шаҳар Ўзбекистон'), 'Shahar Oʻzbekiston'); // default 1995
  assert.equal(toCyrillic('özbek ğalaba şahar çoy'), 'ўзбек ғалаба шаҳар чой');
  assert.equal(toCyrillic('özbek ğalaba şahar çoy', o), 'ўзбек ғалаба шаҳар чой');
  assert.equal(toCyrillic('Oʻzbek Özbek Shahar Şahar'), 'Ўзбек Ўзбек Шаҳар Шаҳар'); // 1995 reads both
  assert.equal(toCyrillic('Oʻzbek shahar', { latinAlphabet: 'auto' }), 'Ўзбек шаҳар'); // no ö ğ ş ç: read as 1995
  assert.equal(toCyrillic('Oʻzbek Özbek', { latinAlphabet: '2026' }), 'Оъзбек Ўзбек'); // 2026 reads o + tutuq
  assert.equal(toLatin('шаҳар', { latinAlphabet: 'auto' }), 'shahar');
});

test('convertRuns: split invariance and unchanged runs', () => {
  const t = "O'zbekiston Respublikasi, shahar";
  for (const to of ['cyrl', 'latn']) {
    const src = to === 'cyrl' ? t : convert(t, 'cyrl');
    for (let i = 0; i <= src.length; i++) {
      for (let j = i; j <= src.length; j += 3) {
        const runs = [src.slice(0, i), src.slice(i, j), src.slice(j)];
        const out = convertRuns(runs, to);
        assert.equal(out.length, 3);
        assert.equal(out.join(''), convert(src, to), JSON.stringify(runs));
      }
    }
  }
  assert.equal(convertRuns(['s', 'h', 'ahar'], 'cyrl').join(''), 'шаҳар');
  assert.equal(convertRuns(['o', "'", 'zbek'], 'cyrl').join(''), 'ўзбек');
});

test('convertRuns: language policy', () => {
  const runs = ['salom ', { text: 'dunyo', lang: 'ru-RU' }, { text: ' tili', lang: 'uz-Latn' }, { text: ' book', lang: 'de' }];
  assert.deepEqual(convertRuns(runs, 'cyrl'), ['салом ', 'дунё', ' тили', ' book']);
  assert.deepEqual(convertRuns(runs, 'cyrl', { langPolicy: 'skip-ru-en' }), ['салом ', 'dunyo', ' тили', ' book']);
});

test('detect, version, errors', () => {
  assert.equal(detect('salom dunyo'), 'latn');
  assert.equal(detect('салом дунё'), 'cyrl');
  assert.equal(detect('123 --'), undefined);
  assert.match(m.version(), /^\d+\.\d+\.\d+$/);
  assert.match(m.rulesVersion(), /^uz-translit-rules\/\d+$/);
  assert.throws(() => convert('a', 'greek'), /direction/);
  assert.throws(() => toLatin('а', { latinAlphabet: '1999' }), /latinAlphabet/);
  assert.throws(() => convertRuns([42], 'latn'), /run/);
});
