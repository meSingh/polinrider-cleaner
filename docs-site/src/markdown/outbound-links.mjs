// Links that leave the site, as the finished HTML has them.
//
// 1. Every one opens in a new tab with rel="noopener", and never
//    "noreferrer" or "nofollow": the sites we link to may see that the visit
//    came from here, and a link here is an endorsement. That part is
//    externalise() in external-links.mjs, unchanged.
// 2. Every page says <meta name="referrer"
//    content="strict-origin-when-cross-origin">, so a click sends our origin
//    (never the page's path) to the other site, whatever the browser's own
//    default.
// 3. Links to what we read and recommend carry ref=polinriderscanner.com,
//    so the other site can count the visits even where the referrer is
//    dropped. That is every outbound link except the hosts in NO_REF: our
//    own repository and its releases, the badges, the author's site, the
//    footer's analytics credit, profile links and package registries. Those
//    are not editorial, and a parameter on a download or an API address
//    helps nobody.
//
// It runs on the built HTML once the build is done, so it covers Markdown,
// MDX, every component and every redirect page with nothing to remember per
// link. On the dev server the same function rewrites each HTML response, so
// a preview shows what the build will publish. scripts/check-external.mjs
// checks the result after every build.
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { externalise } from './external-links.mjs';

export const REFERRER_META = '<meta name="referrer" content="strict-origin-when-cross-origin">';
export const REF = 'ref=polinriderscanner.com';

// A host here, or any subdomain of one, gets no ref parameter.
export const NO_REF = [
  'github.com', // the repository, releases, downloads, attestations, edit links, badges
  'githubusercontent.com',
  'github.io',
  'scorecard.dev', // the OpenSSF Scorecard badge and viewer
  'msingh.com', // the author
  'cloudflare.com', // the footer's analytics credit
  'linkedin.com', // profile links on About
  'twitter.com',
  'registry.npmjs.org', // package registries, linked as places to check a name
  'pypi.org',
];

const OWN = new Set(['polinriderscanner.com', 'www.polinriderscanner.com']);

const noRef = (host) => NO_REF.some((h) => host === h || host.endsWith(`.${h}`));

/** Add ref= to one external href, as written in HTML (&amp; for &). */
function withRef(href) {
  let url;
  try {
    url = new URL(href.replace(/&amp;/g, '&'));
  } catch {
    return href;
  }
  if (url.searchParams.has('ref')) return href;
  const hash = href.indexOf('#');
  const head = hash === -1 ? href : href.slice(0, hash);
  const tail = hash === -1 ? '' : href.slice(hash);
  return `${head}${head.includes('?') ? '&amp;' : '?'}${REF}${tail}`;
}

/** The whole outbound rule for one page. Exported for the check and tests. */
export function outbound(html, origin, base) {
  let out = externalise(html, origin, base);
  // externalise() marks every external link with target="_blank"; those are
  // the ones that may take a ref.
  out = out.replace(/<a\b[^>]*\btarget="_blank"[^>]*>/g, (tag) =>
    tag.replace(/(\shref=")(https?:\/\/[^"]*)(")/i, (m, pre, href, post) => {
      let host;
      try {
        host = new URL(href.replace(/&amp;/g, '&')).hostname;
      } catch {
        return m;
      }
      if (OWN.has(host) || noRef(host)) return m;
      return `${pre}${withRef(href)}${post}`;
    }),
  );
  if (!out.includes('name="referrer"')) {
    if (/<head[\s>]/i.test(out)) out = out.replace(/<head\b[^>]*>/i, (h) => `${h}${REFERRER_META}`);
    // Astro's redirect pages have no <head> tag; a meta right after the
    // doctype still lands in the document's head.
    else out = out.replace(/^(\s*<!doctype html>)?/i, (d) => `${d}${REFERRER_META}`);
  }
  return out;
}

export function outboundLinks({ origin, base }) {
  return {
    name: 'polinrider-outbound-links',
    hooks: {
      'astro:build:done': ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        let pages = 0;
        let links = 0;
        let refs = 0;
        const walk = (d) => {
          for (const f of readdirSync(d)) {
            const p = join(d, f);
            if (statSync(p).isDirectory()) walk(p);
            else if (f.endsWith('.html')) {
              const before = readFileSync(p, 'utf8');
              const after = outbound(before, origin, base);
              if (after !== before) writeFileSync(p, after);
              pages++;
              links += (after.match(/target="_blank"/g) ?? []).length;
              refs += (after.match(/[?&;]ref=polinriderscanner\.com/g) ?? []).length;
            }
          }
        };
        walk(root);
        logger.info(`Outbound links: ${links} open in a new tab on ${pages} pages, ${refs} carry ${REF}.`);
      },

      // The dev server: buffer each HTML response and send it rewritten.
      // Anything that is not HTML passes straight through.
      'astro:server:setup': ({ server }) => {
        server.middlewares.use((req, res, next) => {
          const write = res.write.bind(res);
          const end = res.end.bind(res);
          const writeHead = res.writeHead.bind(res);
          const chunks = [];
          let html;
          const isHtml = () => {
            if (html === undefined) html = /text\/html/i.test(String(res.getHeader('content-type') ?? ''));
            return html;
          };
          const buffer = (chunk, enc) => {
            if (chunk == null || typeof chunk === 'function') return;
            chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, typeof enc === 'string' ? enc : 'utf8'));
          };
          res.writeHead = (status, ...rest) => {
            const headers = rest.find((r) => r && typeof r === 'object');
            const type = headers && Object.entries(headers).find(([k]) => k.toLowerCase() === 'content-type')?.[1];
            if (type) res.setHeader('content-type', type);
            if (isHtml()) {
              res.removeHeader('content-length');
              if (headers) for (const k of Object.keys(headers)) if (k.toLowerCase() === 'content-length') delete headers[k];
            }
            return writeHead(status, ...rest);
          };
          res.write = (chunk, enc, cb) => {
            if (!isHtml()) return write(chunk, enc, cb);
            buffer(chunk, enc);
            if (typeof enc === 'function') enc();
            else if (typeof cb === 'function') cb();
            return true;
          };
          res.end = (chunk, enc, cb) => {
            if (!isHtml()) return end(chunk, enc, cb);
            buffer(chunk, enc);
            const body = Buffer.from(outbound(Buffer.concat(chunks).toString('utf8'), origin, base));
            if (!res.headersSent) res.setHeader('content-length', body.length);
            return end(body, typeof enc === 'function' ? enc : cb);
          };
          next();
        });
      },
    },
  };
}
