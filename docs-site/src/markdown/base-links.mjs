// Adds the site's base path to links written from the site root in
// Markdown: [Order matters](/docs/guides/order/) gets the base in front
// while the site is served under one (GitHub Pages' project path), and
// stays as written at the root of a domain. Content never
// names the base, so moving the site is a one-line change in
// src/siteConfig.mjs. A Satteri hast plugin, for .md and .mdx alike.
//
// Addresses changed before the site was published (November 2026): News is
// Updates, every article lives at /<category>/<slug>/, the weekly reviews at
// /indicator-reviews/ and the threat page is What is PolinRider?. A link
// written to an old form goes to the new address, so content (and the
// weekly reviews, which are never edited) keeps working. `articles` maps an
// article's file id to its address (src/sources.mjs, articlePaths).
export function baseLinks(base, articles = new Map()) {
  const renamed = (value) => {
    const [path, rest = ''] = value.split(/(?=[#?])/);
    let p = path
      .replace(/^\/(news|updates)\/indicator-reviews\//, '/indicator-reviews/')
      .replace(/^\/threat\//, '/what-is-polinrider/');
    const m = /^\/(?:news|updates)\/(\d{4}-\d{2}-\d{2}-[\w-]+)\/$/.exec(p);
    if (m && articles.has(m[1])) p = articles.get(m[1]);
    p = p.replace(/^\/news\//, '/updates/');
    return p + rest;
  };
  const fix = (value) => {
    if (typeof value !== 'string' || !value.startsWith('/') || value.startsWith('//')) return undefined;
    if (base && value.startsWith(`${base}/`)) return undefined;
    const next = `${base}${renamed(value)}`;
    return next === value ? undefined : next;
  };
  return {
    name: 'polinrider-base-links',
    element: [
      {
        filter: ['a', 'img'],
        visit(node, ctx) {
          const key = node.tagName === 'img' ? 'src' : 'href';
          const fixed = fix(node.properties?.[key]);
          if (fixed) ctx.setProperty(node, key, fixed);
        },
      },
    ],
  };
}
