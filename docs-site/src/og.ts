// One social image per update, weekly indicator review and release, and
// one for the threat page. Each card is an HTML page under /og/ (src/pages/
// og) that the build turns into /og/<slug>.jpg and then deletes
// (src/markdown/og-images.mjs). Pages point their og:image here. New posts,
// reviews and releases get a card with no change to anything.
import { getCollection } from 'astro:content';
import { humanDates, newsPosts, reviews } from './news';
import { releases } from './releases';
import { longDate } from './releaseDate';
import { SITE_URL } from './siteConfig.mjs';
import { answer } from './threat-data';

export interface Card {
  slug: string; // updates/2026-11-04-..., updates/indicator-reviews/2026-11-02, releases/v3.0.0, what-is-polinrider
  kind: 'post' | 'review' | 'release' | 'threat';
  eyebrow: string;
  title: string;
  subtitle: string;
}

/** Inline Markdown to plain text, for a one-line subtitle. */
const plain = (s: string) => s.replace(/`([^`]+)`/g, '$1').replace(/\*\*([^*]+)\*\*/g, '$1').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1');

/** The first sentence: a subtitle must end where a sentence ends, never cut. */
const sentence = (s: string) => (s.match(/^(.+?[.!?])(\s|$)/)?.[1] ?? s).trim();
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export async function cards(): Promise<Card[]> {
  const out: Card[] = [];
  for (const p of await newsPosts())
    out.push({ slug: `updates/${p.id}`, kind: 'post', eyebrow: `Update · ${longDate(p.date)}`, title: p.title, subtitle: sentence(p.description) });
  const entries = await getCollection('reviews');
  for (const r of await reviews()) {
    const e = entries.find((x) => x.id === r.id);
    out.push({
      slug: `updates/indicator-reviews/${r.id}`,
      kind: 'review',
      eyebrow: `Weekly indicator review · ${longDate(r.date)}`,
      title: r.outcome ? cap(plain(r.outcome)) : humanDates(e?.data.title ?? r.label),
      subtitle: sentence(plain(r.summary ?? e?.data.description ?? 'Every published source checked again, and what changed in the indicator set and why.')),
    });
  }
  for (const r of await releases())
    out.push({ slug: `releases/${r.id}`, kind: 'release', eyebrow: `Release ${r.version} · ${longDate(r.date)}`, title: (await getCollection('releases')).find((x) => x.id === r.id)!.data.title, subtitle: sentence(r.description) });
  out.push({ slug: 'what-is-polinrider', kind: 'threat', eyebrow: 'What is PolinRider?', title: 'PolinRider: the supply\u2011chain malware', subtitle: answer });
  return out;
}

/** The image a page should name in og:image, or undefined to use the default. */
export function ogImage(slug: string, alt: string) {
  return { url: `${SITE_URL}/og/${slug}.jpg`, width: 1200, height: 630, alt };
}
