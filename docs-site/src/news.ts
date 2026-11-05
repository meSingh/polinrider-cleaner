// What Updates (/updates/) lists: our articles, each in a category and at
// /<category>/<slug>/ (src/categories.mjs); articles published elsewhere,
// also in a category, which link out; and the weekly indicator reviews at
// /indicator-reviews/<date>/. Each is read from its own collection; the
// folders keep their old names (content/news, content/docs/
// indicator-reviews). Newest first throughout.
import { getCollection } from 'astro:content';
import { longDate } from './releaseDate';
import { articlePath, articleSlug, category } from './categories.mjs';

/** Every YYYY-MM-DD in a line written as "2 November 2026". For display
 * only: the reviews themselves are never edited. */
export const humanDates = (s: string) => s.replace(/\b(\d{4}-\d{2}-\d{2})\b/g, (d) => longDate(d));

const base = import.meta.env.BASE_URL.replace(/\/$/, '');

export interface NewsPost {
  id: string; // e.g. 2026-11-04-temp-auto-push-bat
  slug: string; // temp-auto-push-bat
  category: string; // guides
  categoryLabel: string; // Guide
  path: string; // /guides/temp-auto-push-bat/ (no base)
  href: string;
  title: string;
  description: string;
  date: string; // YYYY-MM-DD, the day it was published
  review?: string; // YYYY-MM-DD of the indicator review it came from
  label: string; // a short name, for lists
  minutes: number; // reading time
}

/** Reading time at 220 words a minute, from the Markdown with code and
 * link targets left out. */
export function readingMinutes(body = ''): number {
  const words = body
    .replace(/^import .*$/gm, '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/\]\([^)]*\)/g, ']')
    .replace(/^\[[^\]]+\]:.*$/gm, '')
    .replace(/<[^>]+>/g, ' ')
    .split(/\s+/)
    .filter((w) => /\w/.test(w)).length;
  return Math.max(1, Math.round(words / 220));
}

export async function newsPosts(): Promise<NewsPost[]> {
  return (await getCollection('news'))
    .map(({ id, data, body }) => ({
      id,
      slug: articleSlug(id, data.slug),
      category: data.category,
      categoryLabel: category(data.category)!.label,
      path: articlePath(id, data),
      href: `${base}${articlePath(id, data)}`,
      minutes: readingMinutes(body),
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
      href: `${base}/indicator-reviews/${id}/`,
      date: id.slice(0, 10),
      label: humanDates(rows.get(id)?.label ?? id),
      outcome: rows.get(id)?.outcome,
      summary: rows.get(id)?.summary,
    }))
    .sort((a, b) => (a.id < b.id ? 1 : -1));
}

export async function elsewhere() {
  return (await getCollection('elsewhere'))
    .map(({ id, data }) => ({ id, ...data, categoryLabel: category(data.category)!.label }))
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
  kind: 'update' | 'review' | 'reading';
  /** The category slug, or 'indicator-reviews'; what the Updates tabs filter on. */
  category: string;
  /** The label shown on the entry: "Guide", "Worth reading". */
  label: string;
  date: string;
  title: string;
  /** One line, as HTML (inline code kept). Not shown for a review. */
  summary: string;
  href: string;
  /** Our articles: reading time. */
  minutes?: number;
  /** Worth reading: the publisher, and it opens in a new tab. */
  source?: string;
}

/** A review's compact line: "15 indicators added this week". */
export const reviewLine = (outcome?: string) => {
  const o = (outcome ?? 'Reviewed').replace(/`/g, '');
  return `${o.charAt(0).toUpperCase()}${o.slice(1)} this week`;
};

/** Everything in Updates, newest first: our articles, the weekly reviews and
 * articles published elsewhere, in one line of time. */
export async function timeline(): Promise<TimelineItem[]> {
  const items: TimelineItem[] = [
    ...(await newsPosts()).map((p) => ({
      kind: 'update' as const,
      category: p.category,
      label: p.categoryLabel,
      date: p.date,
      title: p.title,
      summary: inline(p.description),
      href: p.href,
      minutes: p.minutes,
    })),
    ...(await reviews()).map((r) => ({
      kind: 'review' as const,
      category: 'indicator-reviews',
      label: 'Indicator review',
      date: r.date,
      title: reviewLine(r.outcome),
      summary: '',
      href: r.href,
    })),
    ...(await elsewhere()).map((x) => ({
      kind: 'reading' as const,
      category: x.category,
      label: x.categoryLabel,
      date: x.date,
      title: x.title,
      summary: inline(x.summary),
      href: x.url,
      source: x.source,
    })),
  ];
  // Newest first; on the same day our own article comes before a review, and
  // both before an outside article.
  const rank = { update: 0, review: 1, reading: 2 };
  return items.sort((a, b) => (a.date === b.date ? rank[a.kind] - rank[b.kind] : a.date < b.date ? 1 : -1));
}

/** An article's address by its file id, for templates that link one. */
export async function articleHref(id: string): Promise<string> {
  const p = (await newsPosts()).find((x) => x.id === id);
  if (!p) throw new Error(`No article with id ${id}`);
  return p.href;
}
