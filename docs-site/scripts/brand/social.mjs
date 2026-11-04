// Renders the social preview images in headless Chrome (macOS path; set
// CHROME to override). Light: white, ink text, the teal accent line, the
// mark and the product name top left, the domain at the bottom.
//
//   node scripts/brand/social.mjs <variant> <width> <height> <out.jpg|png>
//
// variant "headline": the headline alone, set large across the width.
// variant "checks":   the headline beside the three things it checks.
// variant "row":      the headline large across the width, the three checks
//                     in one row along the bottom.
// public/social-preview.jpg is "headline" at 1200x630.
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const [variant = 'headline', w = '1200', h = '630', out = 'public/social-preview.jpg'] = process.argv.slice(2);
const W = Number(w);
const H = Number(h);
const font = resolve('src/assets/fonts/geist-variable.woff2');
const mark = readFileSync('src/assets/brand/mark.svg', 'utf8').replace(/ role="img" aria-label="[^"]*"/, '');
const icon = (paths) =>
  `<svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
// The icons are the site's own (src/components/Icon.astro).
const checks = [
  // "Your computer", not "This computer": on an image there is no "this".
  ['Your computer', 'macOS, Linux, Windows', '<rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/>'],
  ['Your GitHub account', 'Every repository you own', '<circle cx="12" cy="8" r="4"/><path d="M4.5 20a7.5 7.5 0 0 1 15 0"/>'],
  ['Your organisation', 'Every repository, branch and tag', '<rect x="4" y="3" width="16" height="18" rx="1.5"/><path d="M9 7h1M14 7h1M9 11h1M14 11h1M9 15h1M14 15h1M10 21v-3h4v3"/>'],
];

const pad = Math.round(W * 0.06);
const big = variant === 'headline';
const row = variant === 'row';
const html = `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face { font-family: Geist; src: url('file://${font}') format('woff2'); font-weight: 100 900; }
* { box-sizing: border-box; }
body { margin: 0; }
.c { position: relative; width: ${W}px; height: ${H}px; overflow: hidden; background: #ffffff; color: #0f1220;
  font-family: Geist, system-ui, sans-serif; display: flex; flex-direction: column; padding: ${Math.round(H * 0.1)}px ${pad}px ${Math.round(H * 0.085)}px; }
.c::before { content: ''; position: absolute; inset: 0 0 auto 0; height: 12px; background: #0b6a80; }
.brand { display: flex; align-items: center; gap: 18px; margin: 0; font-size: 34px; font-weight: 650; letter-spacing: -0.02em; }
.brand svg { width: 64px; height: 64px; flex: none; }
.mid { flex: 1; display: flex; align-items: center; gap: 56px; }
.h { margin: 0; font-weight: 750; letter-spacing: -0.04em; line-height: 1.02; font-size: ${big ? 88 : row ? 72 : 64}px; ${big || row ? '' : 'flex: 1;'} }
.row { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; margin: 0 0 ${Math.round(H * 0.045)}px; padding: 0; list-style: none; }
.row li { display: flex; align-items: center; gap: 12px; padding: 14px 16px; border: 2px solid #dfe3ea; border-radius: 16px; background: #f6f8fa; }
.row svg { color: #0b6a80; flex: none; width: 34px; height: 34px; }
.row strong { display: block; font-size: 25px; font-weight: 650; letter-spacing: -0.015em; white-space: nowrap; }
.row small { display: block; margin-top: 2px; color: #474c59; font-size: 18px; white-space: nowrap; }
.h span { display: block; color: #0b6a80; text-wrap: balance; }
.checks { display: grid; gap: 14px; margin: 0; padding: 0; list-style: none; width: ${Math.round(W * 0.34)}px; flex: none; }
.checks li { display: flex; align-items: center; gap: 16px; padding: 16px 20px; border: 2px solid #dfe3ea; border-radius: 16px; background: #f6f8fa; }
.checks svg { color: #0b6a80; flex: none; }
.checks strong { display: block; font-size: 27px; font-weight: 650; letter-spacing: -0.015em; }
.checks small { display: block; margin-top: 2px; color: #474c59; font-size: 20px; }
.foot { display: flex; align-items: baseline; justify-content: space-between; margin: 0; color: #474c59; font-size: 30px; font-weight: 500; }
.foot b { color: #0f1220; font-weight: 650; }
</style></head><body><div class="c">
<p class="brand">${mark}<span>PolinRider Scanner and Cleaner</span></p>
<div class="mid">
  <p class="h">Under attack by PolinRider? <span>Find the malware and remove it safely.</span></p>
  ${big || row ? '' : `<ul class="checks">${checks.map(([t, s, p]) => `<li>${icon(p)}<span><strong>${t}</strong><small>${s}</small></span></li>`).join('')}</ul>`}
</div>
${row ? `<ul class="row">${checks.map(([t, s, p]) => `<li>${icon(p)}<span><strong>${t}</strong><small>${s}</small></span></li>`).join('')}</ul>` : ''}
<p class="foot"><b>polinriderscanner.com</b><span>Free and open source</span></p>
</div></body></html>`;

const chromePath = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const port = 9344;
const chrome = spawn(chromePath, ['--headless=new', `--remote-debugging-port=${port}`, '--hide-scrollbars', '--allow-file-access-from-files', `--user-data-dir=${mkdtempSync(join(tmpdir(), 'social-'))}`, 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let target;
for (let i = 0; i < 50 && !target; i++) {
  await sleep(200);
  try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page'); } catch {}
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const waiting = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (waiting.has(d.id)) { waiting.get(d.id)(d); waiting.delete(d.id); } };
const send = (method, params = {}) => new Promise((r) => { const i = ++id; waiting.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const file = join(mkdtempSync(join(tmpdir(), 'social-html-')), 'card.html');
writeFileSync(file, html);
await send('Page.enable');
await send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: `file://${file}` });
await sleep(800);
await send('Runtime.evaluate', { expression: 'document.fonts.ready', awaitPromise: true });
const jpg = out.endsWith('.jpg');
const shot = await send('Page.captureScreenshot', { format: jpg ? 'jpeg' : 'png', ...(jpg ? { quality: 90 } : {}), clip: { x: 0, y: 0, width: W, height: H, scale: 1 } });
writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
console.log(`${out}: ${variant} ${W}x${H}`);
ws.close();
chrome.kill();
