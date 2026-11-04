// Structured data (schema.org JSON-LD) for every kind of page. Each page
// passes a list of nodes to Base.astro, which writes them as one @graph.
// Everything here describes what the page visibly says: no ratings, no
// claims the page does not make. Nodes refer to each other by @id.
import { REPO_URL, SHORT_TITLE, SITE_DESCRIPTION, SITE_ORIGIN, SITE_TITLE, SITE_URL, SOCIAL_IMAGE } from './siteConfig.mjs';

type Node = Record<string, unknown>;

export const ids = {
  person: `${SITE_URL}/about/#mandeep-singh`,
  website: `${SITE_URL}/#website`,
  software: `${SITE_URL}/#software`,
  source: `${SITE_URL}/#source-code`,
};

/** An absolute address for a path the site links to (base included). */
export const abs = (href: string) => new URL(href, SITE_ORIGIN).href;

/** The author: the project's one maintainer. Facts from his public GitHub
 * profile (github.com/meSingh) and the profiles it links. */
export const person: Node = {
  '@type': 'Person',
  '@id': ids.person,
  name: 'Mandeep Singh',
  url: `${SITE_URL}/about/`,
  jobTitle: 'Fractional CTO and AI architect',
  sameAs: [
    'https://github.com/meSingh',
    'https://msingh.com',
    'https://www.linkedin.com/in/mesingh9/',
    'https://twitter.com/meSingh9',
  ],
};

export const website: Node = {
  '@type': 'WebSite',
  '@id': ids.website,
  name: SITE_TITLE,
  alternateName: SHORT_TITLE,
  url: `${SITE_URL}/`,
  description: SITE_DESCRIPTION,
  inLanguage: 'en-IN',
  publisher: { '@id': ids.person },
};

const offer = { '@type': 'Offer', price: '0', priceCurrency: 'USD' };

/** The program itself, at its latest release. */
export function software(latest: { version: string; date: string }): Node {
  return {
    '@type': 'SoftwareApplication',
    '@id': ids.software,
    name: SITE_TITLE,
    alternateName: [SHORT_TITLE, 'polinrider', 'polinrider-scanner', 'polinrider-cleaner'],
    description: SITE_DESCRIPTION,
    url: `${SITE_URL}/`,
    applicationCategory: 'SecurityApplication',
    applicationSubCategory: 'Malware scanner',
    operatingSystem: 'macOS, Linux, Windows',
    softwareVersion: latest.version,
    datePublished: latest.date,
    downloadUrl: `${REPO_URL}/releases/latest`,
    installUrl: `${SITE_URL}/docs/quick-start/`,
    releaseNotes: `${SITE_URL}/releases/v${latest.version}/`,
    license: 'https://opensource.org/licenses/MIT',
    isAccessibleForFree: true,
    offers: offer,
    image: `${SITE_URL}/icon-512.png`,
    author: { '@id': ids.person },
    publisher: { '@id': ids.person },
  };
}

/** The repository: open source, in Rust, MIT. */
export const sourceCode: Node = {
  '@type': 'SoftwareSourceCode',
  '@id': ids.source,
  name: SITE_TITLE,
  codeRepository: REPO_URL,
  programmingLanguage: 'Rust',
  license: 'https://opensource.org/licenses/MIT',
  author: { '@id': ids.person },
  targetProduct: { '@id': ids.software },
};

/** One release, on its own page. */
export function release(r: { version: string; date: string; description: string; url: string }): Node {
  return {
    '@type': 'SoftwareApplication',
    '@id': `${r.url}#software`,
    name: `${SITE_TITLE} ${r.version}`,
    description: r.description,
    url: r.url,
    applicationCategory: 'SecurityApplication',
    operatingSystem: 'macOS, Linux, Windows',
    softwareVersion: r.version,
    datePublished: r.date,
    downloadUrl: `${REPO_URL}/releases/tag/v${r.version}`,
    releaseNotes: r.url,
    license: 'https://opensource.org/licenses/MIT',
    isAccessibleForFree: true,
    offers: offer,
    author: { '@id': ids.person },
    isVariantOf: { '@type': 'SoftwareApplication', '@id': ids.software, name: SITE_TITLE, url: `${SITE_URL}/` },
  };
}

/** The trail above a page, as BreadcrumbList: Home, the crumbs, the page. */
export function breadcrumbs(crumbs: { label: string; href: string }[], name: string, url: string): Node {
  const base = import.meta.env.BASE_URL.replace(/\/$/, '');
  const trail = [{ label: 'Home', href: `${base}/` }, ...crumbs].map((c) => ({ name: c.label, item: abs(c.href) }));
  if (trail[trail.length - 1]?.item !== url) trail.push({ name, item: url });
  return {
    '@type': 'BreadcrumbList',
    itemListElement: trail.map((t, i) => ({ '@type': 'ListItem', position: i + 1, name: t.name, item: t.item })),
  };
}

/** A page of writing: TechArticle for docs and reference, NewsArticle for
 * News posts. citation is every outside source the page links to. */
export function article(a: {
  type: 'TechArticle' | 'NewsArticle';
  url: string;
  title: string;
  description?: string;
  published?: string;
  modified?: string;
  citations?: string[];
  section?: string;
  image?: string;
}): Node {
  return {
    '@type': a.type,
    '@id': `${a.url}#article`,
    headline: a.title.length > 110 ? `${a.title.slice(0, 107)}...` : a.title,
    description: a.description,
    url: a.url,
    mainEntityOfPage: a.url,
    ...(a.published ? { datePublished: a.published } : {}),
    ...(a.modified ? { dateModified: a.modified } : {}),
    ...(a.section ? { articleSection: a.section } : {}),
    inLanguage: 'en-IN',
    image: a.image ?? SOCIAL_IMAGE.url,
    author: { '@id': ids.person },
    publisher: { '@id': ids.person },
    isPartOf: { '@id': ids.website },
    about: { '@type': 'Thing', name: 'PolinRider', description: 'A supply-chain malware campaign that spreads through developers’ projects.' },
    ...(a.citations?.length ? { citation: a.citations } : {}),
  };
}

/** FAQPage from the questions the page shows, answers as plain text. */
export function faqPage(url: string, items: { q: string; a: string }[]): Node {
  return {
    '@type': 'FAQPage',
    '@id': `${url}#faq`,
    mainEntity: items.map((i) => ({
      '@type': 'Question',
      name: i.q,
      acceptedAnswer: { '@type': 'Answer', text: plain(i.a) },
    })),
  };
}

/** HowTo from steps the page shows. */
export function howTo(url: string, name: string, description: string, steps: { name: string; text: string }[]): Node {
  return {
    '@type': 'HowTo',
    '@id': `${url}#howto`,
    name,
    description,
    tool: [{ '@type': 'HowToTool', name: 'A terminal' }],
    step: steps.map((s, i) => ({ '@type': 'HowToStep', position: i + 1, name: s.name, text: s.text })),
  };
}

/** HTML to plain text, for answers written with links and code. */
export const plain = (html: string) =>
  html
    .replace(/<[^>]+>/g, '')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();

/** Every outside source a Markdown page links to, in order, once each: its
 * citations. Links to this site and to this project's repository are not
 * sources. */
export function citations(markdown = ''): string[] {
  const urls = [
    ...markdown.matchAll(/\]\((https?:\/\/[^)\s]+)\)/g),
    ...markdown.matchAll(/^\[[^\]]+\]:\s*<?(https?:\/\/[^\s>]+)>?/gm),
  ].map((m) => m[1]);
  const own = (u: string) =>
    u.startsWith(SITE_URL) || /^https:\/\/github\.com\/meSingh\//i.test(u) || u.startsWith('https://polinriderscanner.com');
  return [...new Set(urls.filter((u) => !own(u)))];
}

/** The <script type="application/ld+json"> body for a list of nodes. */
export const jsonLd = (nodes: Node[]) =>
  JSON.stringify({ '@context': 'https://schema.org', '@graph': nodes }).replace(/</g, '\\u003c');
