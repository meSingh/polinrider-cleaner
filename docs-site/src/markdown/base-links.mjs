// Adds the site's base path to links written from the site root in
// Markdown: [Order matters](/docs/guides/order/) gets the base in front
// while the site is served under one (GitHub Pages' project path), and
// stays as written at the root of a domain. Content never
// names the base, so moving the site is a one-line change in
// src/siteConfig.mjs. A Satteri hast plugin, for .md and .mdx alike.
export function baseLinks(base) {
  const fix = (value) =>
    typeof value === 'string' && value.startsWith('/') && !value.startsWith('//') && !value.startsWith(`${base}/`)
      ? `${base}${value}`
      : undefined;
  return {
    name: 'polinrider-base-links',
    element: [
      {
        filter: ['a', 'img'],
        visit(node, ctx) {
          const key = node.tagName === 'img' ? 'src' : 'href';
          const fixed = base ? fix(node.properties?.[key]) : undefined;
          if (fixed) ctx.setProperty(node, key, fixed);
        },
      },
    ],
  };
}
