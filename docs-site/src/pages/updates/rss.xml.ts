// The Updates feed, written by hand: an RSS 2.0 document is a few lines of XML,
// which is not worth a dependency. Built once, at build time. Our own posts
// and the weekly indicator reviews, each with its full text
// (content:encoded), so a reader or an agent never needs the page itself.
// Articles published elsewhere are not in the feed.
import type { APIRoute } from 'astro';
import { experimental_AstroContainer as AstroContainer } from 'astro/container';
import { loadRenderers } from 'astro:container';
import { getContainerRenderer as mdxRenderer } from '@astrojs/mdx/container-renderer';
import { getCollection, render } from 'astro:content';
import { newsPosts, reviews } from '../../news';
import { plainArticle } from '../../updates-article';
import { longDate } from '../../releaseDate';
import { SITE_ORIGIN, SITE_TITLE, SITE_URL as site } from '../../siteConfig.mjs';

const escape = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

// RFC 822 dates, as RSS requires. Posts carry a day, not a time.
const rfc822 = (iso: string) => new Date(`${iso}T00:00:00Z`).toUTCString();

// Links in the text are root-relative on the site; a feed reader needs them
// absolute. Scripts and styles are dropped: a feed carries the words.
const forFeed = (html: string) =>
  html
    .replace(/<script[\s\S]*?<\/script>/g, '')
    .replace(/<style[\s\S]*?<\/style>/g, '')
    .replace(/(href|src)="\/(?!\/)/g, `$1="${SITE_ORIGIN}/`)
    .replaceAll(']]>', ']]]]><![CDATA[>');

export const GET: APIRoute = async () => {
  const container = await AstroContainer.create({ renderers: await loadRenderers([mdxRenderer()]) });
  const html = async (entry: Parameters<typeof render>[0]) => {
    const { Content } = await render(entry);
    // The same text as the article page: no boxes, no questions section.
    return forFeed(plainArticle(await container.renderToString(Content, { locals: { plainArticle: true } })));
  };
  const news = await getCollection('news');
  const reviewEntries = await getCollection('reviews');
  const posts = await newsPosts();
  const items: { date: string; xml: string }[] = [];
  for (const p of posts) {
    const url = `${site}${p.path}`;
    items.push({
      date: p.date,
      xml: [
        '    <item>',
        `      <title>${escape(p.title)}</title>`,
        `      <link>${url}</link>`,
        `      <guid isPermaLink="true">${url}</guid>`,
        `      <pubDate>${rfc822(p.date)}</pubDate>`,
        `      <category>${escape(p.categoryLabel)}</category>`,
        `      <description>${escape(p.description)}</description>`,
        `      <content:encoded><![CDATA[${await html(news.find((n) => n.id === p.id)!)}]]></content:encoded>`,
        '    </item>',
      ].join('\n'),
    });
  }
  for (const r of await reviews()) {
    const url = `${site}/indicator-reviews/${r.id}/`;
    const entry = reviewEntries.find((e) => e.id === r.id)!;
    const title = `Weekly indicator review, ${longDate(r.date)}${r.outcome ? `: ${r.outcome}` : ''}`;
    items.push({
      date: r.date,
      xml: [
        '    <item>',
        `      <title>${escape(title)}</title>`,
        `      <link>${url}</link>`,
        `      <guid isPermaLink="true">${url}</guid>`,
        `      <pubDate>${rfc822(r.date)}</pubDate>`,
        '      <category>Indicator review</category>',
        `      <description>${escape(r.summary?.replace(/`/g, '') ?? title)}</description>`,
        `      <content:encoded><![CDATA[${await html(entry)}]]></content:encoded>`,
        '    </item>',
      ].join('\n'),
    });
  }
  items.sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0));

  const body = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>${escape(SITE_TITLE)}: Updates</title>
    <link>${site}/updates/</link>
    <atom:link href="${site}/updates/rss.xml" rel="self" type="application/rss+xml"/>
    <description>What's happening with PolinRider: our updates and the weekly indicator reviews, in full.</description>
    <language>en-IN</language>
${[items[0] ? `    <lastBuildDate>${rfc822(items[0].date)}</lastBuildDate>` : '', items.map((i) => i.xml).join('\n')].filter(Boolean).join('\n')}
  </channel>
</rss>
`;
  return new Response(body, { headers: { 'Content-Type': 'application/rss+xml; charset=utf-8' } });
};
