// The per-page social images. The build writes one HTML card per News post,
// weekly review, release and the threat page under dist/og/ (src/pages/og);
// this step opens each in headless Chrome, saves it as dist/og/<slug>.jpg and
// deletes the HTML, so only the images are published. Static output: nothing
// runs when a page is visited.
//
// Chrome: CHROME if set, else the usual macOS and Linux paths (GitHub's
// Ubuntu runners have Google Chrome); CHROME=none skips drawing, for a
// quicker local build. Without one the build still succeeds:
// every card becomes a copy of the site's default social-preview.jpg and a
// warning says so, so no page ever names a missing image.
import { spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, rmdirSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const candidates = [
  process.env.CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].filter(Boolean);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function browser(path) {
  const port = 9400 + Math.floor(Math.random() * 500);
  const proc = spawn(path, ['--headless=new', '--no-sandbox', `--remote-debugging-port=${port}`, '--hide-scrollbars', '--allow-file-access-from-files', `--user-data-dir=${mkdtempSync(join(tmpdir(), 'og-'))}`, 'about:blank'], { stdio: 'ignore' });
  let target;
  for (let i = 0; i < 75 && !target; i++) {
    await sleep(200);
    try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page'); } catch {}
  }
  if (!target) { proc.kill(); throw new Error('Chrome did not start'); }
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (waiting.has(d.id)) { waiting.get(d.id)(d); waiting.delete(d.id); } };
  const send = (method, params = {}) => new Promise((r) => { const i = ++id; waiting.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1200, height: 630, deviceScaleFactor: 1, mobile: false });
  return { send, close: () => { ws.close(); proc.kill(); } };
}

export function ogImages({ base }) {
  return {
    name: 'polinrider-og-images',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        const root = fileURLToPath(dir).replace(/\/$/, '');
        const ogRoot = join(root, 'og');
        if (!existsSync(ogRoot)) return;
        const pages = [];
        const walk = (d) => { for (const f of readdirSync(d)) { const p = join(d, f); if (statSync(p).isDirectory()) walk(p); else if (f === 'index.html') pages.push(p); } };
        walk(ogRoot);
        const started = Date.now();
        const chrome = process.env.CHROME === 'none' ? undefined : candidates.find((c) => existsSync(c));
        let b;
        try {
          if (chrome) b = await browser(chrome);
        } catch (e) {
          logger.warn(`Chrome would not start (${e.message}).`);
        }
        if (!b) logger.warn('No Chrome: every page uses the default social image. Set CHROME to the browser to draw them.');
        // Root-relative addresses in a card (its font, its stylesheet) become
        // files in dist, so Chrome can open the card from disk.
        const prefix = `${base}/`;
        const local = (html) => html.replace(new RegExp(`(["'(])${prefix.replace(/[/.]/g, '\\$&')}`, 'g'), `$1file://${root}/`);
        for (const page of pages) {
          const slug = relative(ogRoot, dirname(page));
          const out = join(ogRoot, `${slug}.jpg`);
          if (b) {
            const file = join(dirname(page), 'card.html');
            writeFileSync(file, local(readFileSync(page, 'utf8')));
            await b.send('Page.navigate', { url: `file://${file}` });
            await sleep(250);
            await b.send('Runtime.evaluate', { expression: 'document.fonts.ready', awaitPromise: true });
            const shot = await b.send('Page.captureScreenshot', { format: 'jpeg', quality: 90, clip: { x: 0, y: 0, width: 1200, height: 630, scale: 1 } });
            writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
            rmSync(file);
          } else {
            copyFileSync(join(root, 'social-preview.jpg'), out);
          }
          rmSync(page);
          try { rmdirSync(dirname(page)); } catch { /* holds other cards */ }
        }
        b?.close();
        logger.info(`Social images: ${pages.length} in ${((Date.now() - started) / 1000).toFixed(1)}s${b ? '' : ' (default image, no Chrome)'}.`);
      },
    },
  };
}
