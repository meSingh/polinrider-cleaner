// Freeze the documentation for a release (ADR-0056).
//
//   npm run freeze-docs --prefix docs-site -- 2.4.0
//
// Copies src/content/docs/ (without the weekly indicator reviews, which are
// News) to src/content/docs-versions/2.4/, points the copy's links to other
// docs pages at the copy, and records the version in src/docs-versions.json.
// A patch release (2.4.1) replaces the 2.4 copy. Run it in the release's
// pull request, after the docs describe the release; AGENTS.md, "Cutting a
// release", step 1b. The build fails while the newest release has no copy.
//
// It writes files and runs nothing else.
import { cpSync, existsSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const release = process.argv[2] ?? '';
if (!/^\d+\.\d+\.\d+$/.test(release)) {
  console.error('usage: npm run freeze-docs -- X.Y.Z   (the release being cut, e.g. 2.4.0)');
  process.exit(3);
}
const line = release.split('.').slice(0, 2).join('.');
const root = new URL('../src/', import.meta.url).pathname;
const from = join(root, 'content/docs');
const to = join(root, 'content/docs-versions', line);

if (existsSync(to)) rmSync(to, { recursive: true });
cpSync(from, to, {
  recursive: true,
  filter: (src) => !src.startsWith(join(from, 'indicator-reviews')),
});

// Links between docs pages point at the frozen copy; every other link (the
// campaign, News, Releases, GitHub) is left alone.
const walk = (dir) =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
let pages = 0;
for (const file of walk(to).filter((f) => /\.mdx?$/.test(f))) {
  const text = readFileSync(file, 'utf8');
  // Links are written from the site root ([text](/docs/…) or a reference
  // [id]: /docs/…); the base is added at build time.
  writeFileSync(file, text.replace(/(\]\(|\]: )\/docs\//g, `$1/docs/${line}/`));
  pages++;
}

const listFile = join(root, 'docs-versions.json');
const list = JSON.parse(readFileSync(listFile, 'utf8'));
list.frozen = [{ version: line, release }, ...list.frozen.filter((v) => v.version !== line)].sort((a, b) => {
  const [x, y] = [a.version, b.version].map((v) => v.split('.').map(Number));
  return y[0] - x[0] || y[1] - x[1];
});
writeFileSync(listFile, JSON.stringify(list, null, 2) + '\n');

console.log(`freeze-docs: ${pages} pages copied to src/content/docs-versions/${line}/ for ${release}.`);
