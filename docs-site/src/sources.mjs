// Which source files make each page, for its "last modified" date: the
// sitemap's lastmod and the dateModified in its structured data. A page made
// from one file gets that file's last commit; a list gets the newest of the
// files it lists. Paths are relative to docs-site/, without the base.
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { articlePath, categorySlugs } from './categories.mjs';

const one = (dir, id) => [`${dir}/${id}.md`, `${dir}/${id}.mdx`].filter((f) => existsSync(f));

/** The files a page is built from, by its path ('/docs/quick-start/'). */
export function sourcesOf(path) {
  const parts = path.split('/').filter(Boolean);
  if (parts.length === 0) return ['src/pages/index.astro'];
  const [section, ...rest] = parts;
  const id = rest.join('/');
  if (section === 'docs' && !id) return ['src/content/docs', 'src/pages/docs'];
  if (section === 'docs') return one('src/content/docs', id);
  if (section === 'updates' && !id) return ['src/content/news', 'src/content/docs/indicator-reviews', 'src/content/elsewhere.yaml', 'src/content/status.md'];
  if (section === 'indicator-reviews' && !id) return ['src/content/docs/indicator-reviews'];
  if (section === 'indicator-reviews') return one('src/content/docs/indicator-reviews', id);
  if (categorySlugs.includes(section) && !id) return ['src/content/news', 'src/content/elsewhere.yaml'];
  if (categorySlugs.includes(section)) return articleFiles().get(path) ?? [];
  if (section === 'releases' && !id) return ['src/content/releases'];
  if (section === 'releases') return one('src/content/releases', id);
  if (section === 'what-is-polinrider' && !id) return ['src/pages/what-is-polinrider/index.astro'];
  if (section === 'what-is-polinrider') return one('src/content/threat', id);
  if (section === 'about') return ['src/pages/about.astro'];
  return [];
}

const cache = new Map();
/** The day (YYYY-MM-DD) of the last commit touching any of the files, or
 * undefined outside a git checkout or for a page with no known source. */
export function lastCommitDay(files) {
  if (!files.length) return undefined;
  const key = files.join('\n');
  if (!cache.has(key)) {
    let day;
    try {
      day = execFileSync('git', ['log', '-1', '--format=%cs', '--', ...files], {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
      }).trim();
    } catch {
      // Not a git checkout.
    }
    cache.set(key, /^\d{4}-\d{2}-\d{2}$/.test(day ?? '') ? day : undefined);
  }
  return cache.get(key);
}

/** The first commit touching the file: the day a page was first published. */
export function firstCommitDay(file) {
  try {
    const out = execFileSync('git', ['log', '--diff-filter=A', '--follow', '--format=%cs', '--', file], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim().split('\n');
    const day = out[out.length - 1];
    return /^\d{4}-\d{2}-\d{2}$/.test(day) ? day : undefined;
  } catch {
    return undefined;
  }
}

// Each article's address (/<category>/<slug>/) and its file, read from the
// front matter: the same rule the site's routes use (src/categories.mjs).
let files;
function articleFiles() {
  if (files) return files;
  files = new Map();
  for (const [id, path] of articlePaths()) {
    const f = ['md', 'mdx'].map((x) => `src/content/news/${id}.${x}`).find((x) => existsSync(x));
    if (f) files.set(path, [f]);
  }
  return files;
}

/** Every article's file id and its address, without the base, read from the
 * front matter of content/news. */
export function articlePaths(dir = 'src/content/news') {
  const out = new Map();
  for (const f of readdirSync(dir)) {
    if (!/\.mdx?$/.test(f)) continue;
    const text = readFileSync(`${dir}/${f}`, 'utf8');
    const category = /^category:\s*["']?([\w-]+)/m.exec(text)?.[1];
    const slug = /^slug:\s*["']?([\w-]+)/m.exec(text)?.[1];
    const id = f.replace(/\.mdx?$/, '');
    if (category) out.set(id, articlePath(id, { category, slug }));
  }
  return out;
}
