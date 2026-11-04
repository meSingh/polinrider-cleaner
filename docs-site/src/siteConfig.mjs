// Values read both by astro.config.mjs and by the layouts that render every
// page's head and chrome, so each is set in one place.

export const SITE_URL = 'https://mesingh.github.io/polinrider-cleaner';
export const SITE_TITLE = 'PolinRider scanner and cleaner';
export const SITE_DESCRIPTION =
  'Check a machine, a GitHub account or a whole organization for PolinRider malware, ' +
  'and clean it up. Free and open source. Changes nothing until you say yes.';
export const REPO_URL = 'https://github.com/meSingh/polinrider-cleaner';
export const EDIT_URL = `${REPO_URL}/edit/main/docs-site/`;

// Plausible analytics. Off until this holds the site's script ID from the
// Plausible dashboard (Site settings, Site installation), which looks like
// `pa-` followed by letters and digits. While it is empty, nothing is added to
// any page and the footer says nothing about analytics. The ID is public: it
// appears in every page that loads the script.
export const PLAUSIBLE_SCRIPT_ID = '';

// Downloads are release assets: .tar.gz and .zip archives, .deb packages and
// .intoto.jsonl attestations. Setting fileExtensions replaces Plausible's
// default list rather than adding to it. The links point at github.com, so
// outbound link clicks are counted as well.
export const PLAUSIBLE_OPTIONS = {
  fileDownloads: { fileExtensions: ['gz', 'zip', 'deb', 'jsonl'] },
  outboundLinks: true,
};

if (PLAUSIBLE_SCRIPT_ID && !/^pa-[A-Za-z0-9_-]+$/.test(PLAUSIBLE_SCRIPT_ID)) {
  throw new Error(`PLAUSIBLE_SCRIPT_ID is not a Plausible script ID: ${PLAUSIBLE_SCRIPT_ID}`);
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
  alt: 'polinrider-cleaner: a rider inside a cyan lattice between two rows of servers',
};
