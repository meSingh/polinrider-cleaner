// Adds the site's base path to links written from the site root in
// Markdown: [Order matters](/docs/guides/order/) gets the base in front
// while the site is served under one (GitHub Pages' project path), and
// stays as written at the root of a domain. Content never
// names the base, so moving the site is a one-line change in
// src/siteConfig.mjs. A Satteri hast plugin, for .md and .mdx alike.
//
// News became Updates in November 2026: a link written to /news/... goes to
// /updates/... directly, so content (and the weekly reviews, which are
// never edited) keeps working without a redirect hop.
export function baseLinks(base) {
  const renamed = (value) => value.replace(/^\/news\//, '/updates/');
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
