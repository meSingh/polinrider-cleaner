// The day a release came out is the day its signed tag was made, read from
// git when the site is built, so a release page cannot carry the wrong date
// because it was written the day before the tag was cut. The page's own
// `date` stands only until that tag exists. The docs build fetches the tags,
// and the Release workflow rebuilds the site once a release is published.
import { execFileSync } from 'node:child_process';

export function releaseDate(version: string, fallback: string | Date): string {
  // Only a version, never anything else, reaches git.
  if (/^\d+\.\d+\.\d+$/.test(version)) {
    try {
      // The tag's date in UTC, the day GitHub gives the release. A tag cut
      // just after midnight in India is still the previous day here.
      const tagged = execFileSync(
        'git',
        ['for-each-ref', '--format=%(creatordate:format-local:%Y-%m-%d)', `refs/tags/v${version}`],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], env: { ...process.env, TZ: 'UTC' } },
      ).trim();
      if (/^\d{4}-\d{2}-\d{2}$/.test(tagged)) return tagged;
    } catch {
      // Not a git checkout: the page's date stands.
    }
  }
  // YAML reads an unquoted date as a Date; accept either.
  return typeof fallback === 'string' ? fallback : new Date(fallback).toISOString().slice(0, 10);
}

export const longDate = (iso: string) =>
  new Date(`${iso}T00:00:00Z`).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
