// Runs before every build (npm run build): the site's address is written in
// one place, src/siteConfig.mjs, so the site can move to the root of a
// domain by changing one line (ADR-0055). This fails the build when the base
// path or the github.io address is written anywhere else in the site's
// source, content and templates included. Write a link to another page from
// the site root instead: [Order matters](/docs/guides/order/). In a
// component, build it from import.meta.env.BASE_URL.
//
// Past indicator reviews are never edited, so a review that already names
// the address is let through; none does today.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = new URL('..', import.meta.url).pathname;
const repo = 'polinrider' + '-cleaner';
// The base as a path (not after a GitHub owner name or a domain) or the
// github.io address.
const patterns = [new RegExp(`(?<![\\w.-])/${repo}(?![\\w-])`), /mesingh\.github\.io/i];
const allowed = new Set(['src/siteConfig.mjs']);
const pastReview = /^src\/content\/docs\/indicator-reviews\/\d{4}-/;

const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (!['node_modules', 'dist', '.astro'].includes(name)) walk(p);
    } else if (/\.(astro|ts|mjs|js|md|mdx|ya?ml|json|css)$/.test(name)) files.push(relative(root, p));
  }
};
walk(join(root, 'src'));
walk(join(root, 'scripts'));
files.push('astro.config.mjs', 'news-post-template.mdx', 'release-page-template.mdx', 'indicator-review-template.md');

const found = [];
for (const file of files) {
  if (allowed.has(file) || pastReview.test(file)) continue;
  readFileSync(join(root, file), 'utf8')
    .split('\n')
    .forEach((line, i) => {
      if (patterns.some((p) => p.test(line))) found.push(`${file}:${i + 1}: ${line.trim().slice(0, 100)}`);
    });
}

if (found.length) {
  console.error(
    `check-base: the site's address is written outside src/siteConfig.mjs (${found.length}).\n` +
      'Link from the site root, e.g. [text](/docs/guides/order/), or use import.meta.env.BASE_URL.\n  ' +
      found.join('\n  '),
  );
  process.exit(1);
}
console.log(`check-base: ${files.length} files, the address is set only in src/siteConfig.mjs.`);
