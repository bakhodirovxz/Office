// Uzbek Latin <-> Cyrillic transliteration (1995 official Latin alphabet rules).
const OKINA = 'ʻ';   // U+02BB, used in oʻ / gʻ
const TUTUQ = 'ʼ';   // U+02BC, glottal stop (Cyrillic ъ)
const APOS = "'`ʻʼ‘’";

const CYR_VOWELS = 'аеёиоуўэюяы';
const C2L = {
  а:'a', б:'b', в:'v', г:'g', ғ:'g'+OKINA, д:'d', ж:'j', з:'z', и:'i', й:'y',
  к:'k', қ:'q', л:'l', м:'m', н:'n', о:'o', п:'p', р:'r', с:'s', т:'t', у:'u',
  ў:'o'+OKINA, ф:'f', х:'x', ҳ:'h', ч:'ch', ш:'sh', щ:'sh', ы:'i', э:'e',
  ё:'yo', ю:'yu', я:'ya', ь:'', ъ:TUTUQ,
};

const isUpper = ch => ch !== ch.toLowerCase();
const isCyr = ch => /[Ѐ-ӿ]/.test(ch);

function caseLike(out, src, prev, next) {
  if (!isUpper(src)) return out;
  // All-caps context (neighbour is an upper-case letter) -> SH, otherwise Sh.
  const allCaps = (next && isCyr(next) && isUpper(next)) || (prev && isCyr(prev) && isUpper(prev) && !(next && isCyr(next)));
  if (allCaps) return out.toUpperCase();
  return out.charAt(0).toUpperCase() + out.slice(1);
}

// Each converter returns one output string per source character ('' for a
// character swallowed by a digraph), so a paragraph split into runs can be
// rewritten run by run with the context of the whole paragraph.
function cyrToLatChars(text, { plainApostrophe = false } = {}) {
  const outs = new Array(text.length);
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    const low = ch.toLowerCase();
    const prev = text[i - 1], next = text[i + 1];
    const prevLow = prev ? prev.toLowerCase() : '';
    const wordStart = !prev || !isCyr(prev);
    let out;
    if (low === 'е') {
      out = (wordStart || CYR_VOWELS.includes(prevLow) || prevLow === 'ъ' || prevLow === 'ь') ? 'ye' : 'e';
    } else if (low === 'ц') {
      out = (!wordStart && CYR_VOWELS.includes(prevLow)) ? 'ts' : 's';
    } else if (low === 'ъ' && next && 'еёюя'.includes(next.toLowerCase())) {
      out = '';  // объект -> obyekt: the y comes from the vowel itself
    } else if (low === 'ъ' && prevLow === 'ў') {
      out = '';  // мўъжиза -> moʻjiza
    } else if (low === 'с' && next && next.toLowerCase() === 'ҳ') {
      out = 's' + TUTUQ;  // Исҳоқ -> Isʼhoq, keeps s+h apart from sh
    } else if (low in C2L) {
      out = C2L[low];
    } else {
      outs[i] = ch;
      continue;
    }
    out = caseLike(out, ch, prev, next);
    outs[i] = plainApostrophe ? out.replace(/[ʻʼ]/g, "'") : out;
  }
  return outs;
}

const L2C = {
  a:'а', b:'б', d:'д', e:'е', f:'ф', g:'г', h:'ҳ', i:'и', j:'ж', k:'к', l:'л',
  m:'м', n:'н', o:'о', p:'п', q:'қ', r:'р', s:'с', t:'т', u:'у', v:'в', x:'х',
  y:'й', z:'з', c:'к', w:'в',
};
const LAT_VOWELS = 'aeiou';
const isLat = ch => /[A-Za-z]/.test(ch);
const isAp = ch => !!ch && APOS.includes(ch);

function latToCyrChars(text) {
  const n = text.length;
  const outs = new Array(n).fill('');
  for (let i = 0; i < n; i++) {
    const ch = text[i];
    const low = ch.toLowerCase();
    const prev = text[i - 1];
    const c1 = text[i + 1] ? text[i + 1].toLowerCase() : '';
    const c2 = text[i + 2];
    const up = isUpper(ch);
    const at = i;
    const put = s => { outs[at] = up ? s.toUpperCase() : s; };

    if (isAp(ch)) {
      // Apostrophe between letters (not after o/g, handled below) is the tutuq belgisi.
      const prevIsLetter = prev && isLat(prev);
      const nextIsLetter = text[i + 1] && isLat(text[i + 1]);
      if (prevIsLetter && nextIsLetter) outs[i] = (prev && isUpper(prev) && isUpper(text[i + 1])) ? 'Ъ' : 'ъ';
      else if (prevIsLetter && !nextIsLetter && LAT_VOWELS.includes(prev.toLowerCase())) outs[i] = 'ъ'; // mo' -> моъ (rare)
      else outs[i] = ch;
      continue;
    }
    if (!isLat(ch)) { outs[i] = ch; continue; }

    if ((low === 'o' || low === 'g') && isAp(text[i + 1])) {
      put(low === 'o' ? 'ў' : 'ғ'); i++; continue;
    }
    if (low === 's' && c1 === 'h') { put('ш'); i++; continue; }
    if (low === 'c' && c1 === 'h') { put('ч'); i++; continue; }
    if (low === 's' && isAp(text[i + 1]) && c2 && c2.toLowerCase() === 'h') { put('с'); i++; continue; } // Isʼhoq -> Исҳоқ
    if (low === 'y' && c1 && 'oua'.includes(c1)) {
      // yo'q -> йўқ: y + oʻ is й + ў, not ё
      if (c1 === 'o' && isAp(text[i + 2])) { put('й'); continue; }
      put({ o: 'ё', u: 'ю', a: 'я' }[c1]); i++; continue;
    }
    if (low === 'y' && c1 === 'e') { put('е'); i++; continue; }
    if (low === 'e') {
      const wordStart = !prev || !(isLat(prev) || isAp(prev));
      const afterVowel = prev && LAT_VOWELS.includes(prev.toLowerCase());
      put(wordStart || afterVowel ? 'э' : 'е'); continue;
    }
    put(L2C[low]);
  }
  return outs;
}

const cyrToLat = (text, opts) => cyrToLatChars(text, opts).join('');
const latToCyr = text => latToCyrChars(text).join('');

function detect(text) {
  const cyr = (text.match(/[Ѐ-ӿ]/g) || []).length;
  const lat = (text.match(/[A-Za-z]/g) || []).length;
  if (!cyr && !lat) return null;
  return cyr >= lat ? 'cyr' : 'lat';
}

// Leave untouched: URLs, e-mail addresses, bare domains (loyiha.adliya.uz,
// IJRO.GOV.UZ) and codes where a Latin letter touches 3+ digits (BI72373344);
// units such as m3, dm3, BPK5 are still converted.
const SKIP = /(https?:\/\/\S+|www\.\S+|[\w.+-]+@[\w-]+\.[\w.-]+|\b[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.(?:uz|ru|com|org|net|gov|edu|info|io)\b|\b[A-Za-z0-9]*(?:[A-Za-z]\d{3}|\d{3}[A-Za-z])[A-Za-z0-9]*\b)/gi;
function convertChars(text, dir, opts) {
  const outs = [];
  text.split(SKIP).forEach((part, idx) => {
    const parts = idx % 2 ? part.split('') : (dir === 'cyr' ? cyrToLatChars(part, opts) : latToCyrChars(part));
    for (let k = 0; k < parts.length; k++) outs.push(parts[k]);
  });
  return outs;
}

function convert(text, dir, opts) {
  return convertChars(text, dir, opts).join('');
}

// Converts a paragraph given as its run texts; returns the new text of each run.
function convertSegments(segments, dir, opts) {
  const outs = convertChars(segments.join(''), dir, opts);
  let pos = 0;
  return segments.map(seg => {
    const piece = outs.slice(pos, pos + seg.length).join('');
    pos += seg.length;
    return piece;
  });
}

if (typeof module !== 'undefined') module.exports = { cyrToLat, latToCyr, detect, convert, convertSegments };
