// The News posts, read from the files rather than the content collection:
// Starlight's page schema drops fields it does not know, such as `date` and
// `review`. The same approach as the release pages. Newest first.
const files = import.meta.glob('./content/docs/news/*.{md,mdx}', { eager: true }) as Record<
  string,
  {
    frontmatter: {
      title: string;
      description: string;
      date: string | Date;
      review?: string | Date;
      sidebar?: { label?: string };
    };
  }
>;

export interface NewsPost {
  id: string; // e.g. news/2026-11-09-temp-auto-push-bat
  title: string;
  description: string;
  date: string; // YYYY-MM-DD, the day it was published
  review?: string; // YYYY-MM-DD of the indicator review it came from
  label: string;
}

// YAML reads an unquoted date as a Date; accept either.
const day = (d: string | Date) => (d instanceof Date ? d.toISOString().slice(0, 10) : String(d));

export function newsPosts(): NewsPost[] {
  return Object.entries(files)
    .filter(([path]) => !/\/index\.mdx?$/.test(path))
    .map(([path, { frontmatter: f }]) => ({
      id: 'news/' + path.replace(/^.*\//, '').replace(/\.mdx?$/, ''),
      title: f.title,
      description: f.description,
      date: day(f.date),
      review: f.review ? day(f.review) : undefined,
      label: f.sidebar?.label ?? f.title,
    }))
    .sort((a, b) => (a.date === b.date ? a.id.localeCompare(b.id) : a.date < b.date ? 1 : -1));
}

export function newsPost(id: string): NewsPost | undefined {
  return newsPosts().find((p) => p.id === id);
}
