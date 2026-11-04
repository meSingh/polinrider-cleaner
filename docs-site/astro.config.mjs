// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import lucode from 'lucode-starlight';

// Plausible analytics. Off until this holds the site's script ID from the
// Plausible dashboard (Site settings, Site installation), which looks like
// `pa-` followed by letters and digits. While it is empty, nothing is added to
// any page. The ID is public: it appears in every page that loads the script.
const PLAUSIBLE_SCRIPT_ID = '';

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

// The footer. The privacy line appears only while Plausible is switched on,
// so the footer never claims analytics the page does not load.
const footerText =
  'Built by [Mandeep Singh](https://github.com/meSingh) after cleaning up a real ' +
  'PolinRider incident.' +
  (PLAUSIBLE_SCRIPT_ID
    ? ' Visits are counted with [Plausible](https://plausible.io/data-policy), ' +
      'without cookies or personal data.'
    : '');

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

      // Lucode: a shadcn/ui-styled Starlight theme. It supplies the component
      // overrides, tokens and Expressive Code config; everything below is
      // ordinary Starlight configuration on top of it.
      plugins: [
        lucode({
          // Where the site came from. Shown on every page. The licence notices
          // of the theme and of Starlight ship with the site in
          // /third-party-notices.txt; see src/pages/third-party-notices.txt.ts.
          footerText,
          navLinks: [
            { label: 'Quick start', link: '/quick-start/' },
            { label: 'The campaign', link: '/campaign/what-it-is/' },
            { label: 'News', link: '/news/' },
            { label: 'Releases', link: '/releases/' },
          ],
        }),
      ],

      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/meSingh/polinrider-cleaner' },
      ],

      // A correction should be a pull request, not an issue somebody has to
      // translate into one.
      editLink: {
        baseUrl: 'https://github.com/meSingh/polinrider-cleaner/edit/main/docs-site/',
      },

      lastUpdated: true,

      head: [...siteHead, ...plausibleHead(PLAUSIBLE_SCRIPT_ID)],

      customCss: ['./src/styles/custom.css'],

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
