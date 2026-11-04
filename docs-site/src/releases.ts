// The releases, read from their pages, newest first. The date shown is the
// release tag's once it exists: see releaseDate.ts.
import { getCollection } from 'astro:content';
import { releaseDate } from './releaseDate';

const base = import.meta.env.BASE_URL.replace(/\/$/, '');

const parts = (v: string) => v.split('.').map(Number);
export const newerFirst = (a: string, b: string) => {
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return y[i] - x[i];
  return 0;
};

export async function releases() {
  return (await getCollection('releases'))
    .map((e) => ({
      id: e.id,
      href: `${base}/releases/${e.id}/`,
      version: e.data.version,
      date: releaseDate(e.data.version, e.data.date),
      description: e.data.description,
      breaking: e.data.breaking,
    }))
    .sort((a, b) => newerFirst(a.version, b.version));
}
