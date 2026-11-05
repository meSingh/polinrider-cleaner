// The categories an article can belong to. The category is the first part
// of its address: /<slug>/<article>/. Each category in use gets an index
// page at /<slug>/ and a filter tab on Updates. Add a category here and
// nowhere else. label is the one on each entry, title the index page's
// heading, pageTitle its <title>. Plain JavaScript, so the build config can read it too
// (astro.config.mjs, for links written in content).
export const categories = [
  { slug: 'announcements', label: 'Announcement', title: 'Announcements', pageTitle: 'PolinRider Scanner announcements', description: 'News about the PolinRider Scanner project itself: releases, changes and plans.' },
  { slug: 'guides', label: 'Guide', title: 'Guides', pageTitle: 'PolinRider guides', description: 'What to do when you find PolinRider, or something that looks like it, step by step.' },
  { slug: 'analysis', label: 'Analysis', title: 'Analysis', pageTitle: 'PolinRider analysis', description: 'How PolinRider works and how it changes, read closely from the published research.' },
  { slug: 'worth-reading', label: 'Worth reading', title: 'Worth reading', pageTitle: 'Worth reading on PolinRider', description: 'Articles about PolinRider from other researchers and outlets, with a sentence on why each matters.' },
];

export const categorySlugs = categories.map((c) => c.slug);
/** @param {string} slug */
export const category = (slug) => categories.find((c) => c.slug === slug);

/** An article's own part of its address: its file name without the date,
 * or the slug its front matter sets.
 * @param {string} id @param {string} [override] */
export const articleSlug = (id, override) => override ?? id.replace(/^\d{4}-\d{2}-\d{2}-/, '');

/** /<category>/<slug>/ for an article, without the base. */
export const articlePath = (id, data) => `/${data.category}/${articleSlug(id, data.slug)}/`;
