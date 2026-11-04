// Runs after every build (npm run build): every link from one page of the
// site to another must reach a page that exists, an anchor on it that
// exists, or one of the redirects for the old addresses. Astro does not
// check this itself, and a docs site with dead links teaches readers to stop
// following them. Exits 1 and lists every broken link.
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';

const dist = new URL('../dist/', import.meta.url).pathname;
const base = '/polinrider-cleaner';

const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name !== 'pagefind' && name !== '_astro') walk(p);
    } else if (name.endsWith('.html')) files.push(p);
  }
};
walk(dist);

const ids = new Map();
const idsOf = (file) => {
  if (!ids.has(file)) {
    const html = readFileSync(file, 'utf8');
    ids.set(file, new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])));
  }
  return ids.get(file);
};

const target = (path) => {
  const rel = decodeURIComponent(path.slice(base.length)).replace(/^\//, '');
  for (const f of [join(dist, rel), join(dist, rel, 'index.html')]) {
    if (existsSync(f) && statSync(f).isFile()) return f;
  }
  return null;
};

const broken = [];
for (const file of files) {
  const html = readFileSync(file, 'utf8');
  const page = '/' + relative(dist, file).replace(/index\.html$/, '');
  for (const [, href] of html.matchAll(/\shref="([^"]+)"/g)) {
    let url;
    try {
      url = new URL(href.replace(/&amp;/g, '&'), `https://mesingh.github.io${base}${page}`);
    } catch {
      broken.push(`${page}: ${href} (not a URL)`);
      continue;
    }
    if (url.host !== 'mesingh.github.io' || !url.pathname.startsWith(base)) continue;
    const f = target(url.pathname);
    if (!f) {
      broken.push(`${page}: ${href}`);
      continue;
    }
    if (!url.pathname.endsWith('/') && !/\.[a-z0-9]+$/.test(url.pathname)) broken.push(`${page}: ${href} (no trailing slash)`);
    const hash = decodeURIComponent(url.hash.slice(1));
    if (hash && f.endsWith('.html') && !idsOf(f).has(hash) && !readFileSync(f, 'utf8').includes('http-equiv="refresh"')) {
      broken.push(`${page}: ${href} (no #${hash} on that page)`);
    }
  }
}

if (broken.length) {
  console.error(`check-links: ${broken.length} broken internal links\n  ${[...new Set(broken)].join('\n  ')}`);
  process.exit(1);
}
console.log(`check-links: ${files.length} pages, every internal link resolves.`);
