// llms-full.txt: the site's writing in one Markdown file, for a language
// model that wants all of it at once: every live docs page, the indicator set and every update.
import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';
import { facts, flatten, policy } from '../machine';
import { navFor, pagesOf, builtVersions } from '../docs';
import { newsPosts } from '../news';
import { plainArticleMarkdown } from '../updates-article';
import { SITE_URL } from '../siteConfig.mjs';

export const GET: APIRoute = async () => {
  const f = await facts();
  const docs = await pagesOf('');
  const version = (await builtVersions()).find((v) => v.latest)!;
  const order = navFor(version, docs).flatMap((g) => g.items.map((i) => i.id));
  const news = await getCollection('news');
  const posts = await newsPosts();
  const threat = await getCollection('threat');
  const section = (title: string, url: string, body: string) => `# ${title}\n\nSource: ${url}\n\n${body}\n`;
  const parts = [
    `# ${f.name}\n\n> ${f.description}\n\n${policy}\n\nEvery section below names the page it comes from. Dates are YYYY-MM-DD.\n`,
    ...order.map((id) => {
      const e = docs.get(id)!;
      return section(e.data.title, `${SITE_URL}/docs/${id}/`, flatten(e.body ?? ''));
    }),
    ...threat.filter((t) => t.id !== 'index').map((t) => section(t.data.title, `${SITE_URL}/what-is-polinrider/${t.id}/`, flatten(t.body ?? ''))),
    ...posts.map((p) => {
      const e = news.find((n) => n.id === p.id)!;
      return section(p.title, `${SITE_URL}${p.path}`, `Published ${p.date}.\n\n${flatten(plainArticleMarkdown(e.body ?? ''))}`);
    }),
  ];
  return new Response(parts.join('\n---\n\n'), { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
