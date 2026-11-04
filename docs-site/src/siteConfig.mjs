// Values read both by astro.config.mjs and by the components that render the
// page chrome, so each is set in one place.

// Plausible analytics. Off until this holds the site's script ID from the
// Plausible dashboard (Site settings, Site installation), which looks like
// `pa-` followed by letters and digits. While it is empty, nothing is added to
// any page and the footer says nothing about analytics. The ID is public: it
// appears in every page that loads the script.
export const PLAUSIBLE_SCRIPT_ID = '';

export const REPO_URL = 'https://github.com/meSingh/polinrider-cleaner';
