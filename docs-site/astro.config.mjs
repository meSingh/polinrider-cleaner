// @ts-check
// The site is plain Astro: every page's HTML comes from src/layouts and
// src/pages, and the design from src/styles. ADR-0055 says why it left
// Starlight; ADR-0056 how the documentation is versioned.
import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { satteri } from '@astrojs/markdown-satteri';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { asides } from './src/markdown/asides.mjs';
import { baseLinks } from './src/markdown/base-links.mjs';
import { BASE, SITE_ORIGIN } from './src/siteConfig.mjs';
import { pagefindIndex } from './src/markdown/pagefind.mjs';
import { outboundLinks } from './src/markdown/outbound-links.mjs';
import { ogImages } from './src/markdown/og-images.mjs';
import { articlePaths, lastCommitDay, sourcesOf } from './src/sources.mjs';

const src = fileURLToPath(new URL('./src', import.meta.url));

// Every address the site has ever published keeps working (ADR-0055). Each
// old page becomes a small HTML page that sends the reader on, with a
// canonical link to the new address for search engines. GitHub Pages serves
// files only, so these are the redirects there can be.
const moved = (from, to) => [from, to];
const redirects = Object.fromEntries([
  moved('/quick-start/', '/docs/quick-start/'),
  // Only the pages the old site actually had (as of v2.3.0). /guides/ is
  // now also an article category, so a docs page added later must not get
  // a redirect there that could collide with an article's address.
  ...['account', 'ci', 'machine', 'order', 'organization'].map((p) => moved(`/guides/${p}/`, `/docs/guides/${p}/`)),
  ...['exit-codes', 'false-positives', 'verifying'].map((p) => moved(`/reference/${p}/`, `/docs/reference/${p}/`)),
  ...['contributing', 'disclaimer', 'security'].map((p) => moved(`/project/${p}/`, `/docs/project/${p}/`)),
  // The campaign pages became one explainer, What is PolinRider?, in
  // November 2026.
  moved('/campaign/', '/what-is-polinrider/'),
  moved('/campaign/what-it-is/', '/what-is-polinrider/'),
  moved('/campaign/how-it-hides/', '/what-is-polinrider/#why-your-git-history-looks-clean'),
  moved('/campaign/indicators/', '/what-is-polinrider/indicators/'),
]);

// Frozen copies of the documentation (src/content/docs-versions) are kept
// out of the sitemap: /docs/ is the page to find.
const versions = JSON.parse(readFileSync(new URL('./src/docs-versions.json', import.meta.url), 'utf8'));
const frozen = versions.frozen.map((v) => `/docs/${v.version}/`);

// The site's origin and base path come from src/siteConfig.mjs. Every
// internal link is built with the base, or the whole thing 404s once it
// leaves localhost.
// The dev server accepts an address with or without its trailing slash, so
// a review never lands on Astro's own "trailingSlash" notice page; the build
// keeps "always", which is what GitHub Pages serves. Links are always
// written with the slash either way.
const isDev = process.argv.includes('dev');

export default defineConfig({
  site: SITE_ORIGIN,
  base: BASE || '/',
  trailingSlash: isDev ? 'ignore' : 'always',
  // Astro's floating dev toolbar is not part of the site; keep reviews of
  // the dev server looking like the real thing.
  devToolbar: { enabled: false },
  redirects: Object.fromEntries(Object.entries(redirects).map(([from, to]) => [from, `${BASE}${to}`])),

  integrations: [
    mdx(),
    sitemap({
      filter: (page) => {
        const path = new URL(page).pathname.slice(BASE.length);
        // The /og/ cards become images; they are not pages.
        return !(path in redirects) && !frozen.some((f) => path.startsWith(f)) && !path.startsWith('/og/');
      },
      // lastmod from git: the last commit to the files the page is made of.
      serialize: (item) => {
        const lastmod = lastCommitDay(sourcesOf(new URL(item.url).pathname.slice(BASE.length)));
        return lastmod ? { ...item, lastmod } : item;
      },
    }),
    ogImages({ base: BASE }),
    // Links that leave the site: new tab, noopener, the referrer meta and,
    // on editorial links, ref=polinriderscanner.com (src/markdown/outbound-links.mjs).
    outboundLinks({ origin: SITE_ORIGIN, base: BASE }),
    pagefindIndex(),
  ],

  markdown: {
    processor: satteri({
      features: { directive: true },
      mdastPlugins: [asides()],
      // Pages link to each other from the site root, /docs/guides/order/,
      // and the base is added here, so content never names it.
      hastPlugins: [baseLinks(BASE, articlePaths())],
    }),
    // Both themes are written into every code block as CSS variables, and
    // the page's theme picks one (src/styles/prose.css). Both pass WCAG AA
    // on the code background.
    shikiConfig: {
      themes: { light: 'github-light-default', dark: 'github-dark-default' },
      defaultColor: false,
    },
  },

  vite: {
    // `~/components/RunBlock.astro` from any page at any depth, so a frozen
    // copy of the docs imports the same way as the original.
    resolve: { alias: { '~': src } },
  },
});
