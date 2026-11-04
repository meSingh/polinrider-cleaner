// Which source files make each page, for its "last modified" date: the
// sitemap's lastmod and the dateModified in its structured data. A page made
// from one file gets that file's last commit; a list gets the newest of the
// files it lists. Paths are relative to docs-site/, without the base.
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';

const one = (dir, id) => [`${dir}/${id}.md`, `${dir}/${id}.mdx`].filter((f) => existsSync(f));

/** The files a page is built from, by its path ('/docs/quick-start/'). */
export function sourcesOf(path) {
  const parts = path.split('/').filter(Boolean);
  if (parts.length === 0) return ['src/pages/index.astro'];
  const [section, ...rest] = parts;
  const id = rest.join('/');
  if (section === 'docs' && !id) return ['src/content/docs', 'src/pages/docs'];
  if (section === 'docs') return one('src/content/docs', id);
  if (section === 'updates' && !id) return ['src/content/news', 'src/content/docs/indicator-reviews', 'src/content/elsewhere.yaml'];
  if (section === 'updates' && id === 'indicator-reviews') return ['src/content/docs/indicator-reviews'];
  if (section === 'updates' && rest[0] === 'indicator-reviews') return one('src/content/docs/indicator-reviews', rest[1]);
  if (section === 'updates') return one('src/content/news', id);
  if (section === 'releases' && !id) return ['src/content/releases'];
  if (section === 'releases') return one('src/content/releases', id);
  if (section === 'threat' && !id) return ['src/pages/threat/index.astro'];
  if (section === 'threat') return one('src/content/threat', id);
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
