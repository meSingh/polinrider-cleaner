// The project's trust badges, the same set as the README's, drawn by the
// site itself, so the badges load nothing from shields.io or any other site.
//
// Three are live, read when the site is built: CI and Semgrep from the
// GitHub API (the latest finished run of each workflow on main), and the
// OpenSSF Scorecard score from api.scorecard.dev. A live badge says
// "passing" or shows a score only when its fetch succeeded and said so;
// otherwise it shows a neutral label that links to the source. The Docs
// workflow builds the site every day, so they stay current (docs.yml).
import { REPO_URL } from './siteConfig.mjs';

export interface Badge {
  label: string;
  value: string;
  href?: string;
  /** good: a live check that passed. neutral: anything else. */
  tone: 'good' | 'neutral';
  live?: boolean;
}

const repo = 'meSingh/polinrider-scanner';

async function json(url: string, headers: Record<string, string> = {}) {
  try {
    const res = await fetch(url, { headers, signal: AbortSignal.timeout(8000) });
    return res.ok ? await res.json() : undefined;
  } catch {
    return undefined;
  }
}

async function workflow(file: string, name: string): Promise<Badge> {
  const token = process.env.GITHUB_TOKEN;
  const run = await json(
    `https://api.github.com/repos/${repo}/actions/workflows/${file}/runs?branch=main&status=completed&per_page=1`,
    { accept: 'application/vnd.github+json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
  );
  const conclusion = run?.workflow_runs?.[0]?.conclusion;
  const href = `${REPO_URL}/actions/workflows/${file}`;
  return conclusion === 'success'
    ? { label: name, value: 'passing', href, tone: 'good', live: true }
    : { label: name, value: 'see status', href, tone: 'neutral', live: true };
}

async function scorecard(): Promise<Badge> {
  const data = await json(`https://api.scorecard.dev/projects/github.com/${repo}`);
  const href = `https://scorecard.dev/viewer/?uri=github.com/${repo}`;
  return typeof data?.score === 'number'
    ? { label: 'OpenSSF Scorecard', value: data.score.toFixed(1), href, tone: 'good', live: true }
    : { label: 'OpenSSF Scorecard', value: 'see score', href, tone: 'neutral', live: true };
}

let cached: Promise<Badge[]> | undefined;

/** Fetched once per build, however many pages show them. */
export function badges(): Promise<Badge[]> {
  cached ??= (async () => {
    const [ci, semgrep, score] = await Promise.all([workflow('ci.yml', 'CI'), workflow('semgrep.yml', 'Semgrep'), scorecard()]);
    return [
      ci,
      semgrep,
      score,
      { label: 'Built with', value: 'Rust', tone: 'neutral' },
      { label: 'Runtime dependencies', value: '0', tone: 'neutral' },
      { label: 'Platforms', value: 'macOS · Linux · Windows', tone: 'neutral' },
      { label: 'Commits', value: 'signed', href: `${REPO_URL}/commits/main`, tone: 'neutral' },
      { label: 'Releases', value: 'attested provenance', href: `${REPO_URL}/releases/latest`, tone: 'neutral' },
      { label: 'License', value: 'MIT', href: `${REPO_URL}/blob/main/LICENSE`, tone: 'neutral' },
    ];
  })();
  return cached;
}
