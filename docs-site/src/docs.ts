// The documentation: its sidebar, in the order a reader goes through the
// work, and its versions (ADR-0056).
//
//   /docs/        the latest release, from src/content/docs/
//   /docs/X.Y/    a frozen copy made when X.Y was released, from
//                 src/content/docs-versions/X.Y/ (scripts/freeze-docs.mjs)
//   older         the docs folder at that release's tag, on GitHub
import { getCollection, type CollectionEntry } from 'astro:content';
import versions from './docs-versions.json';
import { releases } from './releases';
import { REPO_URL } from './siteConfig.mjs';

const base = import.meta.env.BASE_URL.replace(/\/$/, '');

// Each item is a page id under the version's folder and its sidebar label.
// A page missing from a version is skipped; a page this list does not name
// is added under "More", so no page is ever left out of the sidebar.
export const groups: { label: string; items: [string, string][] }[] = [
  {
    label: 'Install and run',
    items: [
      ['quick-start', 'Quick start'],
      ['reference/verifying', 'Verifying this repository'],
    ],
  },
  {
    label: 'Read the result',
    items: [
      ['reference/exit-codes', 'Exit codes'],
      ['reference/false-positives', 'False positives you will see'],
    ],
  },
  {
    label: 'Clean up, in this order',
    items: [
      ['guides/order', 'Order matters'],
      ['guides/machine', 'A machine'],
      ['guides/account', 'A personal GitHub account'],
      ['guides/organization', 'An organization'],
    ],
  },
  { label: 'Keep it out', items: [['guides/ci', 'Every future push']] },
  {
    label: 'The project',
    items: [
      ['project/contributing', 'Contributing'],
      ['project/security', 'Reporting a vulnerability'],
      ['project/disclaimer', 'What this does not promise'],
    ],
  },
];

export type DocsEntry = CollectionEntry<'docs'> | CollectionEntry<'docsVersions'>;

export interface DocsVersion {
  /** '' for the latest, else 'X.Y'. */
  key: string;
  /** The minor line, X.Y: shown as X.Y.x. */
  line: string;
  /** The release it documents, X.Y.Z. */
  release: string;
  root: string; // <base>/docs/ or <base>/docs/2.3/
  latest: boolean;
}

export async function latestRelease(): Promise<string> {
  return (await releases())[0].version;
}

// Every release line from 2.0 has a copy on the site (ADR-0056).
const FIRST_FROZEN = [2, 0];

/** Every version built into the site, the latest first. */
export async function builtVersions(): Promise<DocsVersion[]> {
  const latest = await latestRelease();
  const [major, minor] = latest.split('.').map(Number);
  const line = `${major}.${minor}`;
  const due = major > FIRST_FROZEN[0] || (major === FIRST_FROZEN[0] && minor >= FIRST_FROZEN[1]);
  if (due && !versions.frozen.some((v) => v.version === line)) {
    throw new Error(
      `The newest release is ${latest}, but its docs are not frozen. ` +
        `Run: npm run freeze-docs --prefix docs-site -- ${latest} (AGENTS.md, "Cutting a release").`,
    );
  }
  return [
    { key: '', line, release: latest, root: `${base}/docs/`, latest: true },
    ...versions.frozen.map((v) => ({
      key: v.version,
      line: v.version,
      release: v.release,
      root: `${base}/docs/${v.version}/`,
      latest: false,
    })),
  ];
}

/** The versions kept on GitHub, at their tags. */
export const githubVersions = versions.github.map((v) => ({
  ...v,
  href: `${REPO_URL}/tree/${v.tag}/docs-site/src/content/docs`,
}));

/** A version's pages, keyed by id without the version folder. */
export async function pagesOf(key: string): Promise<Map<string, DocsEntry>> {
  const map = new Map<string, DocsEntry>();
  if (!key) for (const e of await getCollection('docs')) map.set(e.id, e);
  else
    for (const e of await getCollection('docsVersions'))
      if (e.id.startsWith(`${key}/`)) map.set(e.id.slice(key.length + 1), e);
  return map;
}

export interface NavGroup {
  label: string;
  items: { id: string; label: string; href: string; description?: string }[];
}

export function navFor(version: DocsVersion, pages: Map<string, DocsEntry>): NavGroup[] {
  const listed = new Set<string>();
  const nav = groups
    .map((g) => ({
      label: g.label,
      items: g.items
        .filter(([id]) => pages.has(id))
        .map(([id, label]) => {
          listed.add(id);
          return { id, label, href: `${version.root}${id}/`, description: pages.get(id)!.data.description };
        }),
    }))
    .filter((g) => g.items.length);
  const rest = [...pages.keys()].filter((id) => !listed.has(id)).sort();
  if (rest.length)
    nav.push({
      label: 'More',
      items: rest.map((id) => ({
        id,
        label: pages.get(id)!.data.sidebar?.label ?? pages.get(id)!.data.title,
        href: `${version.root}${id}/`,
        description: pages.get(id)!.data.description,
      })),
    });
  return nav;
}
