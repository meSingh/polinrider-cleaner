// Runs after every build (npm run build), next to check-links.mjs: every
// link that leaves the site must open in a new tab with rel="noopener", and
// must not say "noreferrer" or "nofollow"; every page must carry the
// referrer meta; every editorial link must carry ref=polinriderscanner.com,
// and no link to a host in NO_REF may. src/markdown/outbound-links.mjs makes
// it so; this proves it on the HTML that is published. Exits 1 and lists
// every problem.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { BASE as base, SITE_ORIGIN } from '../src/siteConfig.mjs';
import { NO_REF, REFERRER_META } from '../src/markdown/outbound-links.mjs';

const dist = new URL('../dist/', import.meta.url).pathname;
const site = new URL(SITE_ORIGIN);
const ownHosts = new Set(['polinriderscanner.com', 'www.polinriderscanner.com']);
const own = (u) =>
  ownHosts.has(u.hostname) ||
  (u.origin === site.origin && (base === '' || u.pathname === base || u.pathname.startsWith(`${base}/`)));
const noRef = (host) => NO_REF.some((h) => host === h || host.endsWith(`.${h}`));

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

const problems = [];
let external = 0;
let withRef = 0;
let exempt = 0;
for (const file of files) {
  const page = `/${relative(dist, file)}`.replace(/index\.html$/, '');
  const html = readFileSync(file, 'utf8');
  if (!html.includes(REFERRER_META)) problems.push(`${page}: no referrer meta`);
  for (const [tag] of html.matchAll(/<a\b[^>]*>/g)) {
    const href = /\shref="([^"]*)"/.exec(tag)?.[1];
    if (!href || !/^https?:\/\//i.test(href)) continue;
    let url;
    try {
      url = new URL(href.replace(/&amp;/g, '&'));
    } catch {
      problems.push(`${page}: unreadable link ${href}`);
      continue;
    }
    if (own(url)) continue;
    external++;
    const rel = (/\srel="([^"]*)"/.exec(tag)?.[1] ?? '').split(/\s+/);
    const say = (what) => problems.push(`${page}: ${what}: ${href}`);
    if (!/\starget="_blank"/.test(tag)) say('no target="_blank"');
    if (!rel.includes('noopener')) say('no rel="noopener"');
    if (rel.includes('noreferrer')) say('rel="noreferrer"');
    if (rel.includes('nofollow')) say('rel="nofollow"');
    const hasRef = url.searchParams.get('ref') === 'polinriderscanner.com';
    if (noRef(url.hostname)) {
      exempt++;
      if (hasRef) say('ref= on a link that must not carry it');
    } else if (hasRef) withRef++;
    else say('editorial link without ref=polinriderscanner.com');
  }
}

if (problems.length) {
  console.error(`check-external: ${problems.length} problems`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.log(
  `check-external: ${files.length} pages carry the referrer meta; ${external} external links open in a new tab with noopener, ` +
    `none with noreferrer or nofollow; ${withRef} editorial links carry ref=, ${exempt} to exempt hosts do not.`,
);
