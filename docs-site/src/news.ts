// The three kinds of News: our own posts, the weekly indicator reviews and
// articles published elsewhere. Each is read from its own collection. Newest
// first throughout.
import { getCollection } from 'astro:content';

const base = import.meta.env.BASE_URL.replace(/\/$/, '');

export interface NewsPost {
  id: string; // e.g. 2026-11-04-temp-auto-push-bat
  href: string;
  title: string;
  description: string;
  date: string; // YYYY-MM-DD, the day it was published
  review?: string; // YYYY-MM-DD of the indicator review it came from
  label: string; // a short name, for lists
}

export async function newsPosts(): Promise<NewsPost[]> {
  return (await getCollection('news'))
    .map(({ id, data }) => ({
      id,
      href: `${base}/news/${id}/`,
      title: data.title,
      description: data.description,
      date: data.date,
      review: data.review,
      label: data.sidebar?.label ?? data.title,
    }))
    .sort((a, b) => (a.date === b.date ? a.id.localeCompare(b.id) : a.date < b.date ? 1 : -1));
}

export interface Review {
  id: string; // 2026-10-26-socket
  href: string;
  date: string;
  label: string; // "2026-10-26 (Socket)"
  outcome?: string; // "9 indicators added"
  summary?: string;
}

// Past reviews are never edited, so what each one found is read from the
// table on the reviews overview (indicator-reviews/index.md), which gains a
// row every week: | [label](/…/news/indicator-reviews/<id>/) | outcome | summary |
export async function reviews(): Promise<Review[]> {
  const all = await getCollection('reviews');
  const overview = all.find((e) => e.id === 'index')?.body ?? '';
  const rows = new Map<string, { label: string; outcome: string; summary: string }>();
  for (const m of overview.matchAll(
    /^\|\s*\[([^\]]+)\]\([^)]*\/indicator-reviews\/([^/)]+)\/\)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*$/gm,
  )) {
    rows.set(m[2], { label: m[1], outcome: m[3], summary: m[4] });
  }
  return all
    .filter((e) => e.id !== 'index')
    .map(({ id }) => ({
      id,
      href: `${base}/news/indicator-reviews/${id}/`,
      date: id.slice(0, 10),
      label: rows.get(id)?.label ?? id,
      outcome: rows.get(id)?.outcome,
      summary: rows.get(id)?.summary,
    }))
    .sort((a, b) => (a.id < b.id ? 1 : -1));
}

export async function elsewhere() {
  return (await getCollection('elsewhere'))
    .map(({ id, data }) => ({ id, ...data }))
    .sort((a, b) => (a.date < b.date ? 1 : -1));
}

/** A one-line summary from a Markdown table cell, as HTML: `code` kept. */
export const inline = (s: string) =>
  s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

export interface TimelineItem {
  date: string;
  title: string;
  /** One line, as HTML (inline code kept). */
  summary: string;
  href: string;
  /** Published elsewhere: opens in a new tab, with its publisher's name. */
  source?: string;
}

/** Everything in News, newest first: our posts, the weekly reviews and
 * articles published elsewhere, in one line of time. */
export async function timeline(): Promise<TimelineItem[]> {
  const items: TimelineItem[] = [
    ...(await newsPosts()).map((p) => ({ date: p.date, title: p.title, summary: inline(p.description), href: p.href })),
    ...(await reviews()).map((r) => ({
      date: r.date,
      title: `Weekly indicator review: ${r.outcome ?? r.label}`,
      summary: r.summary ? inline(r.summary) : '',
      href: r.href,
    })),
    ...(await elsewhere()).map((x) => ({
      date: x.date,
      title: x.title,
      summary: inline(x.summary),
      href: x.url,
      source: x.source,
    })),
  ];
  // Newest first; on the same day our own post comes before a review, and
  // both before an outside article.
  const rank = (i: TimelineItem) => (i.source ? 2 : i.title.startsWith('Weekly indicator review') ? 1 : 0);
  return items.sort((a, b) => (a.date === b.date ? rank(a) - rank(b) : a.date < b.date ? 1 : -1));
}
