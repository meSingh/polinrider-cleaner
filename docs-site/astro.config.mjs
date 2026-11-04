// @ts-check
// The site is plain Astro: every page's HTML comes from src/layouts and
// src/pages, and the design from src/styles. ADR-0055 says why it left
// Starlight; ADR-0056 how the documentation is versioned.
import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { satteri } from '@astrojs/markdown-satteri';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { asides } from './src/markdown/asides.mjs';
import { baseLinks } from './src/markdown/base-links.mjs';
import { BASE, SITE_ORIGIN } from './src/siteConfig.mjs';
import { pagefindIndex } from './src/markdown/pagefind.mjs';

const src = fileURLToPath(new URL('./src', import.meta.url));

// Every address the site has ever published keeps working (ADR-0055). Each
// old page becomes a small HTML page that sends the reader on, with a
// canonical link to the new address for search engines. GitHub Pages serves
// files only, so these are the redirects there can be.
const moved = (from, to) => [from, to];
const pages = (dir) =>
  readdirSync(new URL(`./src/content/${dir}/`, import.meta.url))
    .filter((f) => /\.mdx?$/.test(f) && !/^index\./.test(f))
    .map((f) => f.replace(/\.mdx?$/, ''));
const redirects = Object.fromEntries([
  moved('/quick-start/', '/docs/quick-start/'),
  ...pages('docs/guides').map((p) => moved(`/guides/${p}/`, `/docs/guides/${p}/`)),
  ...pages('docs/reference').map((p) => moved(`/reference/${p}/`, `/docs/reference/${p}/`)),
  ...pages('docs/project').map((p) => moved(`/project/${p}/`, `/docs/project/${p}/`)),
  moved('/indicator-reviews/', '/news/indicator-reviews/'),
  ...pages('docs/indicator-reviews').map((p) => moved(`/indicator-reviews/${p}/`, `/news/indicator-reviews/${p}/`)),
]);

// Frozen copies of the documentation (src/content/docs-versions) are kept
// out of the sitemap: /docs/ is the page to find.
const versions = JSON.parse(readFileSync(new URL('./src/docs-versions.json', import.meta.url), 'utf8'));
const frozen = versions.frozen.map((v) => `/docs/${v.version}/`);

// The site's origin and base path come from src/siteConfig.mjs. Every
// internal link is built with the base, or the whole thing 404s once it
// leaves localhost.
export default defineConfig({
  site: SITE_ORIGIN,
  base: BASE || '/',
  trailingSlash: 'always',
  redirects: Object.fromEntries(Object.entries(redirects).map(([from, to]) => [from, `${BASE}${to}`])),

  integrations: [
    mdx(),
    sitemap({
      filter: (page) => {
        const path = new URL(page).pathname.slice(BASE.length);
        return !(path in redirects) && !frozen.some((f) => path.startsWith(f));
      },
    }),
    pagefindIndex(),
  ],

  markdown: {
    processor: satteri({
      features: { directive: true },
      mdastPlugins: [asides()],
      // Pages link to each other from the site root, /docs/guides/order/,
      // and the base is added here, so content never names it.
      hastPlugins: [baseLinks(BASE)],
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
