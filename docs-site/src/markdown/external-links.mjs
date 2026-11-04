// Every link that leaves this site opens in a new tab, with rel="noopener".
// Not noreferrer: the sites we link to may see that the visit came from
// here. Not nofollow: a link here is an endorsement. One rule, applied to
// the built HTML once the build is done, so it covers Markdown, MDX, every
// component (nav, footer, downloads, the News cards) and anything written
// later, with nothing to remember per link. Links within the site, and to
// the site's own domain, are left alone. (The dev server shows links as
// written; the rule runs on `astro build`.)
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ownHosts = new Set(['polinriderscanner.com', 'www.polinriderscanner.com']);

/** Rewrite the <a> tags in one page. Exported for testing. */
export function externalise(html, origin, base) {
  const site = new URL(origin);
  const own = (u) =>
    ownHosts.has(u.hostname) ||
    (u.origin === site.origin && (base === '' || u.pathname === base || u.pathname.startsWith(`${base}/`)));
  return html.replace(/<a\b([^>]*)>/g, (tag, attrs) => {
    const href = /\shref="([^"]*)"/.exec(attrs)?.[1];
    if (!href || !/^https?:\/\//i.test(href)) return tag;
    let url;
    try {
      url = new URL(href.replace(/&amp;/g, '&'));
    } catch {
      return tag;
    }
    if (own(url)) return tag;
    const keep = (/\srel="([^"]*)"/.exec(attrs)?.[1] ?? '')
      .split(/\s+/)
      .filter((t) => t && !['noopener', 'noreferrer', 'nofollow'].includes(t));
    const rest = attrs.replace(/\s+target="[^"]*"/, '').replace(/\s+rel="[^"]*"/, '');
    return `<a${rest} target="_blank" rel="${['noopener', ...keep].join(' ')}">`;
  });
}

export function externalLinks({ origin, base }) {
  return {
    name: 'polinrider-external-links',
    hooks: {
      'astro:build:done': ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        let pages = 0;
        let links = 0;
        const walk = (d) => {
          for (const f of readdirSync(d)) {
            const p = join(d, f);
            if (statSync(p).isDirectory()) walk(p);
            else if (f.endsWith('.html')) {
              const before = readFileSync(p, 'utf8');
              const after = externalise(before, origin, base);
              if (after !== before) {
                writeFileSync(p, after);
                pages++;
                links += (after.match(/target="_blank"/g) ?? []).length;
              }
            }
          }
        };
        walk(root);
        logger.info(`External links open in a new tab: ${links} on ${pages} pages.`);
      },
    },
  };
}
