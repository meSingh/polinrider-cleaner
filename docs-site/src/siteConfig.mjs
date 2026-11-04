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
export const PRODUCTION_ORIGIN = 'https://polinriderscanner.com';
const DEFAULT_ORIGIN = ON_OWN_DOMAIN ? PRODUCTION_ORIGIN : 'https://mesingh.github.io';
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

// Cloudflare Web Analytics, in its JS-snippet mode (the domain is DNS-only,
// not proxied). The token is from the Cloudflare dashboard (Analytics &
// Logs, Web Analytics) and is public: it appears in every page that loads
// the beacon. The beacon counts page views without cookies.
export const CF_WEB_ANALYTICS_TOKEN = '916d6aa6c9094d828ac628070245e65c';

// Only the real site is counted: a production build for PRODUCTION_ORIGIN
// at the root gets Cloudflare's snippet exactly as Cloudflare gives it. Dev
// servers, local builds for any other address and the GitHub Pages site get
// no beacon.
export const ANALYTICS_ON =
  Boolean(CF_WEB_ANALYTICS_TOKEN) && SITE_ORIGIN === PRODUCTION_ORIGIN && BASE === '' && process.env.NODE_ENV === 'production';

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
  width: 1200,
  height: 630,
  alt: 'PolinRider Scanner and Cleaner. Under attack by PolinRider? Find the malware and remove it safely. polinriderscanner.com',
};
