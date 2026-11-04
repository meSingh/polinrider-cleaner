// The News feed, written by hand: an RSS 2.0 document is a few lines of XML,
// which is not worth a dependency. Built once, at build time. Our own posts
// only: indicator reviews and articles elsewhere are not in the feed.
import type { APIRoute } from 'astro';
import { newsPosts } from '../../news';
import { SITE_TITLE, SITE_URL as site } from '../../siteConfig.mjs';


const escape = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

// RFC 822 dates, as RSS requires. Posts carry a day, not a time.
const rfc822 = (iso: string) => new Date(`${iso}T00:00:00Z`).toUTCString();

export const GET: APIRoute = async () => {
  const posts = await newsPosts();
  const items = posts
    .map((p) => {
      const url = `${site}/news/${p.id}/`;
      return [
        '    <item>',
        `      <title>${escape(p.title)}</title>`,
        `      <link>${url}</link>`,
        `      <guid isPermaLink="true">${url}</guid>`,
        `      <pubDate>${rfc822(p.date)}</pubDate>`,
        `      <description>${escape(p.description)}</description>`,
        '    </item>',
      ].join('\n');
    })
    .join('\n');

  const body = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>${escape(SITE_TITLE)}: News</title>
    <link>${site}/news/</link>
    <atom:link href="${site}/news/rss.xml" rel="self" type="application/rss+xml"/>
    <description>What changed in the PolinRider supply-chain campaign, and what it means if you are cleaning up.</description>
    <language>en-IN</language>
${[posts[0] ? `    <lastBuildDate>${rfc822(posts[0].date)}</lastBuildDate>` : '', items].filter(Boolean).join('\n')}
  </channel>
</rss>
`;
  return new Response(body, { headers: { 'Content-Type': 'application/rss+xml; charset=utf-8' } });
};
