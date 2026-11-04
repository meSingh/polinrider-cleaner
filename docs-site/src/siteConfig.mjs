// Values read both by astro.config.mjs and by the layouts that render every
// page's head and chrome, so each is set in one place.

// Where the site is served. This is the only place the address is written
// (ADR-0055, "Moving to a root domain"); every link, asset, redirect, the
// feed, the sitemap and the social card are built from these. To serve the
// site from the root of a domain, set BASE to '' (and SITE_ORIGIN to that
// domain). SITE_BASE and SITE_ORIGIN in the environment override both for
// one build, which is how the root build is tested.
//
// The move to polinriderscanner.com (decided for 3.0) is ready: set
// ON_OWN_DOMAIN to true when the domain is live. Until then the site stays
// at its GitHub Pages address. Steps for the switch: ADR-0055.
const ON_OWN_DOMAIN = false;
const DEFAULT_ORIGIN = ON_OWN_DOMAIN ? 'https://polinriderscanner.com' : 'https://mesingh.github.io';
const DEFAULT_BASE = ON_OWN_DOMAIN ? '' : '/polinrider-cleaner';
export const SITE_ORIGIN = process.env.SITE_ORIGIN ?? DEFAULT_ORIGIN;
export const BASE = process.env.SITE_BASE ?? DEFAULT_BASE;
export const SITE_URL = `${SITE_ORIGIN}${BASE}`;
// The product's name, as Mandeep chose it: title case, "and" written out.
export const SITE_TITLE = 'PolinRider Scanner and Cleaner';
export const SITE_DESCRIPTION =
  'Check a machine, a GitHub account or a whole organization for PolinRider malware, ' +
  'and clean it up. Free and open source. Changes nothing until you say yes.';
// The repository, renamed for 3.0. GitHub redirects the old name.
export const REPO_URL = 'https://github.com/meSingh/polinrider-scanner';
export const EDIT_URL = `${REPO_URL}/edit/main/docs-site/`;

// Cloudflare Web Analytics. Off until this holds the site's token from the
// Cloudflare dashboard (Analytics & Logs, Web Analytics, the JS snippet's
// "token" value). While it is empty, nothing is added to any page and the
// footer says nothing about analytics. With it set, every page loads
// Cloudflare's beacon script, which counts page views without cookies. The
// token is public: it appears in every page that loads the beacon.
export const CF_WEB_ANALYTICS_TOKEN = '';

if (CF_WEB_ANALYTICS_TOKEN && !/^[A-Za-z0-9]+$/.test(CF_WEB_ANALYTICS_TOKEN)) {
  throw new Error(`CF_WEB_ANALYTICS_TOKEN is not a Cloudflare Web Analytics token: ${CF_WEB_ANALYTICS_TOKEN}`);
}

// Search engine ownership checks. Each is the `content` value of the meta tag
// that Google Search Console (URL-prefix property, HTML tag method) or Bing
// Webmaster Tools (HTML Meta Tag method) asks for. Empty adds nothing.
export const GOOGLE_SITE_VERIFICATION = '';
export const BING_SITE_VERIFICATION = '';

// The social card every page carries.
export const SOCIAL_IMAGE = {
  url: `${SITE_URL}/social-preview.jpg`,
  width: 1280,
  height: 640,
  alt: 'PolinRider Scanner and Cleaner: a rider inside a cyan lattice between two rows of servers',
};
