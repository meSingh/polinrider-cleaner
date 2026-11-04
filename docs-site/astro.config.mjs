// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { PLAUSIBLE_SCRIPT_ID, REPO_URL } from './src/siteConfig.mjs';

// Plausible analytics: the script ID is set in src/siteConfig.mjs, because
// the footer reads it too.

// Downloads are release assets: .tar.gz and .zip archives, .deb packages and
// .intoto.jsonl attestations. Setting fileExtensions replaces Plausible's
// default list rather than adding to it. The links point at github.com, so
// outbound link clicks are counted as well.
const plausibleOptions = {
  fileDownloads: { fileExtensions: ['gz', 'zip', 'deb', 'jsonl'] },
  outboundLinks: true,
};

function plausibleHead(id) {
  if (!id) return [];
  if (!/^pa-[A-Za-z0-9_-]+$/.test(id)) {
    throw new Error(`PLAUSIBLE_SCRIPT_ID is not a Plausible script ID: ${id}`);
  }
  return [
    { tag: 'script', attrs: { async: true, src: `https://plausible.io/js/${id}.js` } },
    {
      tag: 'script',
      content:
        'window.plausible=window.plausible||function(){(plausible.q=plausible.q||[]).push(arguments)},' +
        'plausible.init=plausible.init||function(i){plausible.o=i||{}};' +
        `plausible.init(${JSON.stringify(plausibleOptions)})`,
    },
  ];
}

// Search engine ownership checks. Each is the `content` value of the meta tag
// that Google Search Console (URL-prefix property, HTML tag method) or Bing
// Webmaster Tools (HTML Meta Tag method) asks for. Empty adds nothing.
const GOOGLE_SITE_VERIFICATION = '';
const BING_SITE_VERIFICATION = '';

const siteUrl = 'https://mesingh.github.io/polinrider-cleaner';

// Tags every page carries besides Starlight's own: the social card, the News
// feed and, once their values exist, the two verification tags. Starlight
// already writes og:title, og:description and twitter:card, but no image.
const siteHead = [
  { tag: 'meta', attrs: { property: 'og:image', content: `${siteUrl}/social-preview.jpg` } },
  { tag: 'meta', attrs: { property: 'og:image:width', content: '1280' } },
  { tag: 'meta', attrs: { property: 'og:image:height', content: '640' } },
  {
    tag: 'meta',
    attrs: {
      property: 'og:image:alt',
      content: 'polinrider-cleaner: a rider inside a cyan lattice between two rows of servers',
    },
  },
  { tag: 'meta', attrs: { name: 'twitter:image', content: `${siteUrl}/social-preview.jpg` } },
  {
    tag: 'link',
    attrs: {
      rel: 'alternate',
      type: 'application/rss+xml',
      title: 'PolinRider news',
      href: `${siteUrl}/news/rss.xml`,
    },
  },
  ...(GOOGLE_SITE_VERIFICATION
    ? [{ tag: 'meta', attrs: { name: 'google-site-verification', content: GOOGLE_SITE_VERIFICATION } }]
    : []),
  ...(BING_SITE_VERIFICATION
    ? [{ tag: 'meta', attrs: { name: 'msvalidate.01', content: BING_SITE_VERIFICATION } }]
    : []),
];

// The site is published to GitHub Pages at /polinrider-cleaner/, so every
// internal link has to be built with that base or the whole thing 404s once
// it leaves localhost.
export default defineConfig({
  site: 'https://mesingh.github.io',
  base: '/polinrider-cleaner',
  trailingSlash: 'always',

  integrations: [
    starlight({
      title: 'PolinRider scanner and cleaner',
      favicon: '/favicon.svg',
      description:
        'Check a machine, a GitHub account or a whole organization for PolinRider malware, ' +
        'and clean it up. Free and open source. Changes nothing until you say yes.',

      // The look is plain Starlight: its own colour, type and layout tokens
      // set in src/styles/theme.css, and a few components replaced where the
      // design needs different markup. Each override says why at its top.
      components: {
        Header: './src/components/overrides/Header.astro',
        SiteTitle: './src/components/overrides/SiteTitle.astro',
        ThemeSelect: './src/components/overrides/ThemeSelect.astro',
        Hero: './src/components/overrides/Hero.astro',
        PageTitle: './src/components/overrides/PageTitle.astro',
        Footer: './src/components/overrides/Footer.astro',
      },

      // Code blocks take their colours from the theme's own tokens, so they
      // follow light and dark with the rest of the page.
      expressiveCode: {
        useStarlightUiThemeColors: true,
        styleOverrides: {
          borderRadius: '14px',
          borderColor: 'var(--pr-border)',
          codeBackground: 'var(--pr-code-bg)',
          codeFontFamily: 'var(--__sl-font-mono)',
          codeFontSize: 'var(--sl-text-code)',
          codeLineHeight: '1.7',
          codePaddingBlock: '1rem',
          codePaddingInline: '1.25rem',
          frames: {
            frameBoxShadowCssValue: 'none',
            editorBackground: 'var(--pr-code-bg)',
            terminalBackground: 'var(--pr-code-bg)',
            terminalTitlebarBackground: 'var(--pr-code-bg)',
            terminalTitlebarBorderBottomColor: 'var(--pr-border)',
            terminalTitlebarDotsOpacity: '0',
            editorTabBarBackground: 'var(--pr-code-bg)',
            editorActiveTabBackground: 'var(--pr-code-bg)',
            editorTabBarBorderBottomColor: 'var(--pr-border)',
            inlineButtonBackground: 'var(--pr-heading)',
            inlineButtonForeground: 'var(--pr-heading)',
            inlineButtonBorder: 'var(--pr-border-strong)',
            tooltipSuccessBackground: 'var(--pr-clean)',
            tooltipSuccessForeground: 'var(--pr-bg)',
          },
        },
      },

      social: [{ icon: 'github', label: 'GitHub', href: REPO_URL }],

      // A correction should be a pull request, not an issue somebody has to
      // translate into one.
      editLink: {
        baseUrl: 'https://github.com/meSingh/polinrider-cleaner/edit/main/docs-site/',
      },

      lastUpdated: true,

      head: [...siteHead, ...plausibleHead(PLAUSIBLE_SCRIPT_ID)],

      customCss: ['./src/styles/theme.css', './src/styles/custom.css'],

      // Releases and indicator reviews each show their own list in the
      // sidebar. See src/routeData.ts.
      routeMiddleware: './src/routeData.ts',

      sidebar: [
        { label: 'Quick start', link: '/quick-start/' },
        {
          label: 'The campaign',
          items: [
            { label: 'What PolinRider is', link: '/campaign/what-it-is/' },
            { label: 'Why your history looks clean', link: '/campaign/how-it-hides/' },
            { label: 'The indicator set', link: '/campaign/indicators/' },
          ],
        },
        {
          label: 'Cleaning up',
          items: [
            { label: 'Order matters', link: '/guides/order/' },
            { label: 'A machine', link: '/guides/machine/' },
            { label: 'A personal GitHub account', link: '/guides/account/' },
            { label: 'An organization', link: '/guides/organization/' },
            { label: 'Every future push', link: '/guides/ci/' },
          ],
        },
        // Three growing lists, each with its own index page rather than a
        // sidebar group that would grow by one entry a week.
        { label: 'Indicator reviews', link: '/indicator-reviews/' },
        { label: 'News', link: '/news/' },
        { label: 'Releases', link: '/releases/' },
        {
          label: 'Reference',
          items: [
            { label: 'Exit codes', link: '/reference/exit-codes/' },
            { label: 'False positives you will see', link: '/reference/false-positives/' },
            { label: 'Verifying this repository', link: '/reference/verifying/' },
          ],
        },
        {
          label: 'The project',
          items: [
            { label: 'Contributing', link: '/project/contributing/' },
            { label: 'Reporting a vulnerability', link: '/project/security/' },
            { label: 'What this does not promise', link: '/project/disclaimer/' },
          ],
        },
      ],
    }),
  ],
});
