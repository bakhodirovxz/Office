// Types a Latin and a Cyrillic word with Gboard (soft key taps, the globe key for the Russian layout; no keyevent) into the focused editor of the Android app.
// Usage: node scripts/mobile/android-type.mjs <serial> <latin> <cyrillic>. Key maps: scripts/mobile/lib.mjs (1080x2400, Gboard 15.1, the Docs phone layout).
import * as L from './lib.mjs';
const [S, latin, cyr] = process.argv.slice(2);
const tapKeys = async (w, map) => { for (const c of w) { const k = c === ' ' ? L.KEY.space : map[c]; if (!k) throw new Error('no key ' + c); L.tap(S, k[0], k[1]); await L.sleep(250); } };
await tapKeys(' ' + latin + ' ', L.EN_LAYOUT); // the first key after the keyboard comes up is swallowed: a space goes first
L.tap(S, ...L.KEY.globe); await L.sleep(1200);
await tapKeys(cyr, L.RU_LAYOUT);
L.tap(S, ...L.KEY.globe); await L.sleep(800);
