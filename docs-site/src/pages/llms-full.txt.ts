// llms-full.txt: the site's writing in one Markdown file, for a language
// model that wants all of it at once. The threat page's key facts and
// questions, every live docs page, the indicator set and every update.
import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';
import { facts, flatten, policy } from '../machine';
import { navFor, pagesOf, builtVersions } from '../docs';
import { newsPosts } from '../news';
import { threatFaq, falseAlarmFaq } from '../faq';
import { plain } from '../schema';
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
  const qa = (items: { q: string; a: string }[]) => items.map((i) => `**${i.q}**\n${plain(i.a)}`).join('\n\n');
  const parts = [
    `# ${f.name}\n\n> ${f.description}\n\n${policy}\n\nEvery section below names the page it comes from. Dates are YYYY-MM-DD.\n`,
    section('The PolinRider threat: questions people ask', `${SITE_URL}/threat/`, qa(threatFaq)),
    ...order.map((id) => {
      const e = docs.get(id)!;
      return section(e.data.title, `${SITE_URL}/docs/${id}/`, flatten(e.body ?? ''));
    }),
    ...threat.filter((t) => t.id !== 'index').map((t) => section(t.data.title, `${SITE_URL}/threat/${t.id}/`, flatten(t.body ?? ''))),
    ...posts.map((p) => {
      const e = news.find((n) => n.id === p.id)!;
      const extra = p.id === '2026-11-04-polinrider-or-false-alarm' ? `\n\n## Questions people ask\n\n${qa(falseAlarmFaq)}` : '';
      return section(p.title, `${SITE_URL}/updates/${p.id}/`, `Published ${p.date}.\n\n${flatten(e.body ?? '')}${extra}`);
    }),
  ];
  return new Response(parts.join('\n---\n\n'), { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
