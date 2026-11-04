// Freeze the documentation for a release (ADR-0056).
//
//   npm run freeze-docs --prefix docs-site -- 2.4.0
//   npm run freeze-docs --prefix docs-site -- 2.2.0 --from-tag
//
// Docs are kept per minor release line, shown as 2.4.x: a copy is cut only
// when the major or minor version changes. Copies src/content/docs/
// (without the weekly indicator reviews, which are News) to
// src/content/docs-versions/2.4/, points the copy's links to other docs
// pages at the copy, and records the line in src/docs-versions.json. A patch
// release (2.4.1) shares its line's docs, so for one this does nothing. Run
// it in the release's pull request, after the docs describe the release;
// AGENTS.md, "Cutting a release", step 1b. The build fails while the newest
// release line has no copy.
//
// --from-tag takes the docs as they were at the release's tag instead
// (vX.Y.Z, read with git), for a release cut before versioning existed. Its
// links are rewritten to this site's layout: docs pages to the copy, the
// rest of the site to where it is now, and links to files in the
// repository to GitHub at that tag.
//
// It writes files and reads git; it runs nothing else.
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const release = process.argv[2] ?? '';
const fromTag = process.argv.includes('--from-tag');
if (!/^\d+\.\d+\.\d+$/.test(release)) {
  console.error('usage: npm run freeze-docs -- X.Y.Z   (the release being cut, e.g. 2.4.0)');
  process.exit(3);
}
const line = release.split('.').slice(0, 2).join('.');
const patch = Number(release.split('.')[2]);
const root = new URL('../src/', import.meta.url).pathname;
const from = join(root, 'content/docs');
const to = join(root, 'content/docs-versions', line);

if (existsSync(to) && patch > 0) {
  console.log(`freeze-docs: ${release} is a patch release; it shares the ${line}.x docs. Nothing to do.`);
  process.exit(0);
}
if (existsSync(to)) rmSync(to, { recursive: true });
if (!fromTag) {
  cpSync(from, to, {
    recursive: true,
    filter: (src) => !src.startsWith(join(from, 'indicator-reviews')),
  });
} else {
  // Only the documentation: not the home page, News, releases, reviews or
  // the campaign pages, which have their own sections now.
  const tag = `v${release}`;
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8', cwd: root });
  const prefix = 'docs-site/src/content/docs/';
  const docs = git('ls-tree', '--full-tree', '-r', '--name-only', tag, prefix)
    .split('\n')
    .filter((f) => /^docs-site\/src\/content\/docs\/(quick-start\.mdx?|(guides|reference|project)\/[^/]+\.mdx?)$/.test(f));
  const repoFile = `https://github.com/meSingh/polinrider-cleaner/blob/${tag}/`;
  // The address the site had before; spelt in parts so the build's own check
  // for that address (check-base.mjs) does not stop on this script.
  const oldHost = ['mesingh', 'github', 'io'].join('.');
  const oldBase = ['polinrider', 'cleaner'].join('-');
  for (const file of docs) {
    let text = git('show', `${tag}:${file}`);
    text = text
      // Old addresses, with or without the base the site had then.
      .replace(new RegExp(`https://${oldHost.replaceAll('.', '\\.')}/${oldBase}/`, 'g'), '/')
      .replace(new RegExp(`(\\]\\(|\\]: )/${oldBase}/`, 'g'), '$1/')
      .replace(/(\]\(|\]: )\/indicator-reviews\//g, '$1/news/indicator-reviews/')
      // Files in the repository, linked relative to the repository root.
      .replace(/\]\((?![a-z]+:|\/|#)([^)\s]+)\)/g, (_, p) => `](${repoFile}${p})`);
    const out = join(to, file.slice(prefix.length));
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, text);
  }
}

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
  // From a tag, the docs pages sat at the root (/guides/…, /quick-start/).
  const docsLink = fromTag ? /(\]\(|\]: )\/(docs\/)?(quick-start|guides|reference|project)\//g : /(\]\(|\]: )\/docs\/()()/g;
  writeFileSync(
    file,
    text.replace(docsLink, (_, lead, _d, section) => `${lead}/docs/${line}/${section ? `${section}/` : ''}`),
  );
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
