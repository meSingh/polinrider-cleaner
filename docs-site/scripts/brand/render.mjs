// Renders the mark's PNG icons and favicon.ico from the SVGs that
// scripts/brand/mark.py writes, in headless Chrome (macOS path; set CHROME
// to override). Run from docs-site: node scripts/brand/render.mjs
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const chromePath = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const port = 9341;
const chrome = spawn(chromePath, ['--headless=new', `--remote-debugging-port=${port}`, '--hide-scrollbars', `--user-data-dir=${mkdtempSync(join(tmpdir(), 'mark-'))}`, 'about:blank'], { stdio: 'ignore' });
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

async function png(svgFile, size) {
  const svg = readFileSync(svgFile, 'utf8').replace('<svg ', `<svg width="${size}" height="${size}" `);
  const html = `<!doctype html><html><body style="margin:0;background:transparent">${svg}</body></html>`;
  await send('Emulation.setDeviceMetricsOverride', { width: size, height: size, deviceScaleFactor: 1, mobile: false });
  await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } });
  await send('Page.navigate', { url: `data:text/html;base64,${Buffer.from(html).toString('base64')}` });
  await sleep(300);
  const s = await send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: size, height: size, scale: 1 } });
  return Buffer.from(s.result.data, 'base64');
}

// An .ico holding PNG images, which every current browser reads.
function ico(images) {
  const head = Buffer.alloc(6 + 16 * images.length);
  head.writeUInt16LE(0, 0); head.writeUInt16LE(1, 2); head.writeUInt16LE(images.length, 4);
  let offset = head.length;
  images.forEach(([size, data], i) => {
    const e = 6 + 16 * i;
    head.writeUInt8(size % 256, e); head.writeUInt8(size % 256, e + 1);
    head.writeUInt16LE(1, e + 4); head.writeUInt16LE(32, e + 6);
    head.writeUInt32LE(data.length, e + 8); head.writeUInt32LE(offset, e + 12);
    offset += data.length;
  });
  return Buffer.concat([head, ...images.map(([, d]) => d)]);
}

await send('Page.enable');
const small = 'public/favicon.svg';
const master = 'src/assets/brand/mark.svg';
writeFileSync('public/favicon.ico', ico([[16, await png(small, 16)], [32, await png(small, 32)]]));
writeFileSync('public/apple-touch-icon.png', await png('src/assets/brand/apple-touch-icon.svg', 180));
writeFileSync('public/icon-192.png', await png(master, 192));
writeFileSync('public/icon-512.png', await png(master, 512));
console.log('favicon.ico, apple-touch-icon.png, icon-192.png, icon-512.png');
ws.close();
chrome.kill();
