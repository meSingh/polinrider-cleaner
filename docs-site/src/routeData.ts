// Two sections get a sidebar of their own. On a release page the sidebar is
// the list of releases, and on an indicator review it is the list of reviews,
// newest first, with a link back to the rest of the documentation. Every other
// page keeps the sidebar configured in astro.config.mjs, where each of these
// sections is a single link: both lists grow, one of them weekly.
import { getCollection } from 'astro:content';
import { defineRouteMiddleware, type StarlightRouteData } from '@astrojs/starlight/route-data';

type Entry = StarlightRouteData['sidebar'][number];

const releasePages = import.meta.glob('./content/docs/releases/v*.{md,mdx}', { eager: true }) as Record<
  string,
  { frontmatter: { version: string; date?: string | Date } }
>;

const base = import.meta.env.BASE_URL.replace(/\/$/, '');

const link = (label: string, href: string, current: string, badge?: string): Entry => ({
  type: 'link',
  label,
  href,
  isCurrent: href === current,
  attrs: {},
  ...(badge ? { badge: { text: badge, variant: 'tip' } } : {}),
});

const group = (label: string, entries: Entry[]): Entry => ({
  type: 'group',
  label,
  entries,
  collapsed: false,
  badge: undefined,
});

// "12 Oct 2026": short enough to sit beside a version in the sidebar.
const short = (iso: string) =>
  new Date(`${iso}T00:00:00Z`).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    timeZone: 'UTC',
  });

const versionParts = (v: string) => v.split('.').map(Number);
const newerVersionFirst = (a: string, b: string) => {
  const [x, y] = [versionParts(a), versionParts(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return y[i] - x[i];
  return 0;
};

export const onRequest = defineRouteMiddleware(async (context) => {
  const route = context.locals.starlightRoute;
  const id = route.entry.id;
  const current = context.url.pathname;
  const back = link('← All documentation', `${base}/quick-start/`, current);

  if (id === 'releases' || id.startsWith('releases/')) {
    // Read from the files: Starlight's page schema drops fields it does not
    // know, such as `version` and `date`.
    const releases = Object.values(releasePages)
      .map(({ frontmatter: f }) => ({
        version: String(f.version),
        date: f.date instanceof Date ? f.date.toISOString().slice(0, 10) : f.date,
      }))
      .sort((a, b) => newerVersionFirst(a.version, b.version));
    route.sidebar = [
      back,
      group('Releases', [
        link('All releases', `${base}/releases/`, current),
        ...releases.map((r, i) =>
          link(
            r.date ? `${r.version} · ${short(r.date)}` : r.version,
            `${base}/releases/v${r.version}/`,
            current,
            i === 0 ? 'latest' : undefined,
          ),
        ),
      ]),
    ];
  } else if (id === 'indicator-reviews' || id.startsWith('indicator-reviews/')) {
    const dates = (await getCollection('docs'))
      .map((e) => e.id.match(/^indicator-reviews\/(\d{4}-\d{2}-\d{2})$/)?.[1])
      .filter((d): d is string => Boolean(d))
      .sort()
      .reverse();
    route.sidebar = [
      back,
      group('Indicator reviews', [
        link('All reviews', `${base}/indicator-reviews/`, current),
        ...dates.map((d, i) => link(d, `${base}/indicator-reviews/${d}/`, current, i === 0 ? 'latest' : undefined)),
      ]),
    ];
  }
});
