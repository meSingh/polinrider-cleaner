// The facts that llms.txt, llms-full.txt and /data/polinrider.json give to
// language models and agents, in one place. All of it is read from the site's
// own content or restates it: nothing here is said only to machines.
import { getCollection } from 'astro:content';
import { pagesOf, navFor, builtVersions } from './docs';
import { newsPosts, reviews } from './news';
import { releases } from './releases';
import { REPO_URL, SITE_DESCRIPTION, SITE_TITLE, SITE_URL } from './siteConfig.mjs';

const repoPath = REPO_URL.replace('https://github.com/', '');
export const raw = (file: string) => `https://raw.githubusercontent.com/${repoPath}/main/${file}`;

/** The indicator files in ioc/, as the threat page and ioc/README.md list them. */
export const indicatorFiles = [
  { file: 'strong.txt', meaning: 'Text only the malware leaves', effect: 'compromised' },
  { file: 'strong-patterns.txt', meaning: 'Patterns in file contents, where a part rotates', effect: 'compromised' },
  { file: 'bad-packages.txt', meaning: 'Malicious package names from the campaign', effect: 'compromised' },
  { file: 'filenames.txt', meaning: 'File names and paths that are signs on their own', effect: 'compromised' },
  { file: 'hashes.txt', meaning: 'SHA-256 hashes of known campaign files, whatever they are named', effect: 'compromised' },
  { file: 'ssh-keys.txt', meaning: 'Attacker SSH keys added to authorized_keys', effect: 'compromised' },
  { file: 'network.txt', meaning: 'Campaign hosts and IP addresses, if a live connection matches', effect: 'compromised' },
  { file: 'implant-paths.txt', meaning: 'Paths of the related implant, if they exist', effect: 'compromised' },
  { file: 'implant-names.txt', meaning: 'Implant program names, on an exact process-name match', effect: 'compromised' },
  { file: 'filenames-review.txt', meaning: 'File names a normal project can also have', effect: 'review' },
  { file: 'weak.txt', meaning: 'Text a normal project can also have', effect: 'review' },
];

/** The four answers, with their exit codes (docs/reference/exit-codes.md). */
export const verdicts = [
  { exitCode: 0, verdict: 'clean', meaning: 'None of the current indicators is present on what was checked. Not proof that nothing was ever there: rotate credentials anyway if you had a reason to check.' },
  { exitCode: 1, verdict: 'review', meaning: 'Nothing confirmed. Some lines name things a normal project can also have, or a check that could not run; a person has to decide.' },
  { exitCode: 2, verdict: 'compromised', meaning: 'At least one confirmed indicator: something only the malware leaves. Check the computer first, then GitHub.' },
  { exitCode: 3, verdict: 'could not run', meaning: 'The check could not run (a missing path, dependency, indicator set or GitHub login). It says nothing either way.' },
];

export const install = {
  homebrew: 'brew install meSingh/tap/polinrider-scanner',
  scoop: 'scoop bucket add mesingh https://github.com/meSingh/scoop-bucket\nscoop install polinrider-scanner',
  run: 'polinrider',
};

export const policy =
  'Everything on this site is public. People, search engines, AI assistants and agents may read, quote and cite it; please link to the page you quote. The indicator set and the code are MIT licensed.';

const downloadTargets = [
  ['macOS, Apple silicon', 'macos-arm64.tar.gz'],
  ['macOS, Intel', 'macos-x86_64.tar.gz'],
  ['Linux x86_64', 'linux-x86_64.tar.gz'],
  ['Linux x86_64, Debian and Ubuntu', 'linux-x86_64.deb'],
  ['Linux arm64', 'linux-arm64.tar.gz'],
  ['Linux arm64, Debian and Ubuntu', 'linux-arm64.deb'],
  ['Windows x86_64', 'windows-x86_64.zip'],
  ['Windows x86_64', 'windows-x86_64.tar.gz'],
] as const;

export async function facts() {
  const latest = (await releases())[0];
  const tag = `v${latest.version}`;
  const dl = (f: string) => `${REPO_URL}/releases/download/${tag}/${f}`;
  const docs = await pagesOf('');
  const version = (await builtVersions()).find((v) => v.latest)!;
  const nav = navFor(version, docs);
  const posts = await newsPosts();
  const allReviews = await reviews();
  const threat = await getCollection('threat');
  return {
    name: SITE_TITLE,
    description: SITE_DESCRIPTION,
    site: `${SITE_URL}/`,
    repository: REPO_URL,
    license: 'MIT',
    author: { name: 'Mandeep Singh', url: `${SITE_URL}/about/`, github: 'https://github.com/meSingh' },
    policy,
    latestRelease: {
      version: latest.version,
      date: latest.date,
      notes: `${SITE_URL}/releases/${tag}/`,
      github: `${REPO_URL}/releases/tag/${tag}`,
      downloads: downloadTargets.map(([system, f]) => ({ system, file: `polinrider-${tag}-${f}`, url: dl(`polinrider-${tag}-${f}`) })),
      checksums: dl('SHA256SUMS'),
      attestation: dl(`polinrider-${tag}.intoto.jsonl`),
      verify: [
        'sha256sum -c SHA256SUMS --ignore-missing',
        `gh attestation verify polinrider-${tag}-macos-arm64.tar.gz --repo ${repoPath}`,
      ],
      verifyDocs: `${SITE_URL}/docs/reference/verifying/`,
    },
    install: { ...install, docs: `${SITE_URL}/docs/quick-start/` },
    verdicts,
    indicatorSet: {
      folder: `${REPO_URL}/tree/main/ioc`,
      page: `${SITE_URL}/what-is-polinrider/indicators/`,
      lastReview: allReviews[0] ? { date: allReviews[0].date, url: `${SITE_URL}/updates/indicator-reviews/${allReviews[0].id}/` } : undefined,
      reviews: `${SITE_URL}/updates/indicator-reviews/`,
      files: indicatorFiles.map((f) => ({ ...f, url: raw(`ioc/${f.file}`) })),
    },
    pages: {
      threat: { title: 'What is PolinRider malware?', url: `${SITE_URL}/what-is-polinrider/` },
      about: { title: 'About', url: `${SITE_URL}/about/` },
      docs: nav.flatMap((g) =>
        g.items.map((i) => ({ group: g.label, title: docs.get(i.id)!.data.title, description: i.description, url: `${SITE_URL}/docs/${i.id}/` })),
      ),
      reference: threat.filter((t) => t.id !== 'index').map((t) => ({ title: t.data.title, description: t.data.description, url: `${SITE_URL}/what-is-polinrider/${t.id}/` })),
      updates: posts.map((p) => ({ title: p.title, description: p.description, date: p.date, url: `${SITE_URL}/updates/${p.id}/` })),
    },
    feeds: { rss: `${SITE_URL}/updates/rss.xml`, sitemap: `${SITE_URL}/sitemap-index.xml`, llms: `${SITE_URL}/llms.txt`, llmsFull: `${SITE_URL}/llms-full.txt` },
  };
}

/** Markdown as plain readable text for llms-full.txt: imports dropped,
 * components unwrapped, site links made absolute. */
export function flatten(md: string) {
  return md
    .replace(/^import .*$/gm, '')
    .replace(/<RunBlock[^>]*\/>/g, '```bash\n' + install.homebrew + '   # macOS and Linux\n' + install.scoop + '   # Windows\n```')
    .replace(/<Faq [^>]*\/>/g, '')
    .replace(/<[A-Z][A-Za-z]*\b[^>]*\/>/g, '')
    .replace(/<\/?[A-Z][A-Za-z]*\b[^>]*>/g, '')
    .replace(/<\/?div[^>]*>/g, '')
    .replace(/^:::(\w+)(\[([^\]]*)\])?\s*$/gm, (_m, kind, _b, title) => (title ? `**${title}**` : kind === undefined ? '' : ''))
    .replace(/^:::\s*$/gm, '')
    .replace(/\]\(\/(?!\/)/g, `](${SITE_URL}/`)
    .replace(/^(\[[^\]]+\]:\s*)\/(?!\/)/gm, `$1${SITE_URL}/`)
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
