// WP7 diagnostic: log focus()/blur() calls on eframe's text agent (and focus/blur events) inside the Docs frame while a swipe happens.
//   RIG_SCALE=0.5 node scripts/mobile/kbd-probe.mjs   (session.mjs running, document open, keyboard hidden with the chevron)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import * as L from './lib.mjs';
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const pwDir = [path.join(ROOT, 'ci/web-smoke'), process.env.PLAYWRIGHT_DIR, 'E:/office-wt/phase-5/ci/web-smoke'].find((d) => d && fs.existsSync(path.join(d, 'node_modules')));
const { chromium } = createRequire(pwDir + '/')('@playwright/test');
const ser = L.serial('HRM_360');
const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
const page = browser.contexts()[0].pages().find((p) => p.url().startsWith('http://localhost'));
const frame = page.frames().find((f) => f.url().includes('/docs/'));
await frame.evaluate(() => {
  const a = document.querySelector('input[type=text]'); window.__log = [];
  const t0 = performance.now(), note = (s) => window.__log.push(`${Math.round(performance.now() - t0)} ${s}`);
  for (const m of ['focus', 'blur']) { const o = HTMLElement.prototype[m]; a[m] = function (...x) { note(`${m}() called, hadFocus=${document.activeElement === a}`); return o.apply(this, x); }; }
  const cv = document.querySelector('canvas'), cf = cv.focus; cv.focus = function (...x) { note('canvas.focus() called'); return cf.apply(this, x); };
  for (const e of ['focus', 'blur']) a.addEventListener(e, (ev) => note(`event ${e} related=${ev.relatedTarget?.tagName ?? '-'} active=${document.activeElement?.tagName}`));
  for (const e of ['focus', 'blur']) cv.addEventListener(e, () => note(`canvas event ${e}`));
  for (const e of ['touchstart', 'touchend', 'touchcancel']) addEventListener(e, () => note(`${e} active=${document.activeElement?.tagName}`), true);
  for (const e of ['pointerdown', 'pointerup', 'pointercancel', 'mousedown', 'mouseup', 'click', 'contextmenu']) addEventListener(e, () => note(e), true);
  const vd = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
  Object.defineProperty(a, 'value', { get() { return vd.get.call(this); }, set(v) { note(`value=${JSON.stringify(v)}`); vd.set.call(this, v); }, configurable: true });
  for (const m of ['setSelectionRange', 'select', 'setAttribute', 'removeAttribute']) { const o = a[m]; a[m] = function (...x) { note(`${m}(${x.join(',')})`); return o.apply(this, x); }; }
  for (const e of ['selectionchange']) document.addEventListener(e, () => note(e));
  for (const e of ['beforeinput', 'input', 'compositionstart', 'compositionend', 'keydown']) a.addEventListener(e, () => note(`input-event ${e}`));
  visualViewport.addEventListener('resize', () => note(`vv resize h=${Math.round(visualViewport.height)}`));
  addEventListener('resize', () => note(`window resize h=${innerHeight}`));
  window.__probe = a.outerHTML.slice(0, 80);
});
const shown = () => /mInputShown=true/.test(L.sh(ser, 'dumpsys input_method'));
console.log('keyboard shown before:', shown());
const what = process.argv[2] ?? 'swipe';
if (what === 'swipe') L.swipe(ser, 600, 1500, 600, 700, 400); else if (what === 'chevron') L.tap(ser, 106, 2326); else L.tap(ser, 600, 1300);
await L.sleep(4000);
console.log(await frame.evaluate(() => window.__log.join('\n') + '\nactive=' + document.activeElement?.tagName));
console.log('keyboard shown:', /mInputShown=true/.test(L.sh(ser, 'dumpsys input_method')));
await browser.close().catch(() => {});
