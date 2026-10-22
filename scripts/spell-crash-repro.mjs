// scripts/spell-crash-repro.mjs   (Phase 4 WP6)
// Reproduces, in the real Slides window, the STATUS_ACCESS_VIOLATION that the two `squiggles_*` unit tests show in a multi-threaded debug
// test binary (docs/phase1/tests.md): with dictionaries installed, type misspelt words into two text boxes, one character at a time and
// switching between the boxes, while the background spell checker settles. One app process per run, driven over the control channel.
//
//   node scripts/spell-crash-repro.mjs <slides.exe> <dictionaries-dir> [runs=20] [lang=en-US|ru-RU] [out-dir]
//
// A run "crashes" when the process exits (any code) before the script asked it to quit, or the control channel dies. The exit code
// 3221225477 (0xC0000005) is the access violation. Prints one line per run and a summary; exit code 1 when any run crashed.
import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as os from 'node:os';
import * as path from 'node:path';

const [exe, dicts, runsArg, langArg, outArg] = process.argv.slice(2);
if (!exe || !dicts) { console.error('usage: node scripts/spell-crash-repro.mjs <slides.exe> <dictionaries-dir> [runs] [lang] [out-dir]'); process.exit(2); }
const RUNS = Number(runsArg ?? 20), LANG = langArg ?? 'en-US';
const out = outArg ?? fs.mkdtempSync(path.join(os.tmpdir(), 'spell-crash-'));
fs.mkdirSync(out, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Misspellings of common words (the dictionary must flag them), then a few right ones.
const TEXT = {
  'en-US': ['Our plna for teh quarter ', 'reveiw the slidse and  recieve feedbak ', 'adress the totl numbres befor monday '],
  'ru-RU': ['Мы подготвили презинтацию ', 'проверте цыфры перед встречей ', 'исправте ошыбки в тексте слайдов '],
}[LANG] ?? ['teh plna '];

async function oneRun(run) {
  const port = 41000 + run, appdata = path.join(out, `appdata-${run}`);
  fs.mkdirSync(appdata, { recursive: true });
  const env = { ...process.env, DECKCRAFT_DICTIONARIES: dicts, APPDATA: appdata, LOCALAPPDATA: appdata };
  const child = spawn(exe, ['--control', String(port)], { env, stdio: 'ignore' });
  let exited = null, asked = false;
  child.on('exit', (code, sig) => { exited = { code, sig }; });
  const log = [];
  try {
    let sock;
    for (let i = 0; i < 200 && !sock && !exited; i++) {
      sock = await new Promise((ok) => { const s = net.connect(port, '127.0.0.1', () => ok(s)); s.on('error', () => ok(null)); });
      if (!sock) await sleep(300);
    }
    if (!sock) throw new Error(exited ? `exited at start (${JSON.stringify(exited)})` : 'no control channel');
    let buf = '', id = 0; const waiting = new Map();
    sock.on('data', (d) => { buf += d; let i; while ((i = buf.indexOf('\n')) >= 0) { const line = buf.slice(0, i); buf = buf.slice(i + 1); try { const m = JSON.parse(line); waiting.get(m.id)?.(m); waiting.delete(m.id); } catch {} } });
    sock.on('close', () => { for (const f of waiting.values()) f({ ok: false, error: 'closed' }); waiting.clear(); });
    const rpc = (method, params = {}) => new Promise((ok) => {
      const n = ++id; waiting.set(n, ok);
      sock.write(JSON.stringify({ id: n, method, params }) + '\n');
      setTimeout(() => { if (waiting.delete(n)) ok({ ok: false, error: 'timeout' }); }, 30000);
    });
    const exec = (command, params = {}) => rpc('engine.execute', { command, params });

    await exec('file.new');
    await exec('slide.layout', { layout: 'blank' });
    const boxes = [];
    for (const [i, rect] of [[0, [60, 80, 420, 160]], [1, [500, 80, 420, 160]]]) {
      const r = await exec('shape.insert', { preset: 'rect', rect, text: '' });
      if (!r.ok) throw new Error('shape.insert: ' + r.error);
      boxes.push(r.result.id);
      if (LANG !== 'en-US') { await exec('edit.select', { ids: [r.result.id] }); await exec('format.language', { lang: LANG }); }
    }
    log.push(`boxes ${boxes.join(',')}`);
    let typed = 0;
    for (let round = 0; round < 3; round++) {
      for (const phrase of TEXT) {
        for (const box of boxes) {
          await exec('edit.select', { ids: [box] });
          await exec('text.edit');
          for (const ch of phrase) {
            const r = await rpc('ui.text', { text: ch });
            if (!r.ok && exited) throw new Error('process gone while typing');
            typed++;
            await sleep(15 + Math.floor(Math.random() * 40));
          }
          if (exited) throw new Error('process gone');
        }
      }
    }
    await sleep(2500); // let the last checks settle
    if (exited) throw new Error('process gone while settling');
    const shot = await rpc('ui.screenshot', { path: path.join(out, `run-${run}.png`) });
    log.push(`typed ${typed} chars, screenshot ${shot.ok}`);
    asked = true;
    await rpc('app.quit');
    for (let i = 0; i < 20 && !exited; i++) await sleep(250);
    if (!exited) child.kill();
    return { run, crashed: false, detail: log.join('; ') };
  } catch (e) {
    await sleep(500);
    const crashed = exited && !asked;
    if (!exited) child.kill();
    return { run, crashed: !!crashed, detail: `${e.message}${exited ? ' exit ' + JSON.stringify(exited) + ' (0xC0000005 = 3221225477)' : ''}; ${log.join('; ')}` };
  }
}

let crashes = 0;
for (let run = 1; run <= RUNS; run++) {
  const r = await oneRun(run);
  if (r.crashed) crashes++;
  console.log(`${r.crashed ? 'CRASH' : 'ok   '} run ${run}/${RUNS} ${LANG}: ${r.detail}`);
}
console.log(`SUMMARY ${crashes} of ${RUNS} GUI runs crashed (${LANG}, dictionaries ${dicts}, out ${out})`);
process.exit(crashes ? 1 : 0);
