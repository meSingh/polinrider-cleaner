// Adds the site's base path to links written from the site root in
// Markdown: [Order matters](/docs/guides/order/) gets the base in front
// while the site is served under one (GitHub Pages' project path), and
// stays as written at the root of a domain. Content never
// names the base, so moving the site is a one-line change in
// src/siteConfig.mjs. A Satteri hast plugin, for .md and .mdx alike.
//
// Two sections were renamed before the site was published (November 2026):
// News is Updates and the threat page is What is PolinRider?. A link written
// to /news/... or /threat/... goes to the new address, so content (and the
// weekly reviews, which are never edited) keeps working.
export function baseLinks(base) {
  const renamed = (value) => value.replace(/^\/news\//, '/updates/').replace(/^\/threat\//, '/what-is-polinrider/');
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
