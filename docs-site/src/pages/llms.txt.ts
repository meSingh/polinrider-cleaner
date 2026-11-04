// llms.txt (llmstxt.org): what this site is, in one screen, with links to
// the pages a language model should read. Built from the site's content.
import type { APIRoute } from 'astro';
import { facts, policy } from '../machine';
import { SITE_URL } from '../siteConfig.mjs';

export const GET: APIRoute = async () => {
  const f = await facts();
  const link = (title: string, url: string, note?: string) => `- [${title}](${url})${note ? `: ${note}` : ''}`;
  const groups = [...new Set(f.pages.docs.map((d) => d.group))];
  const body = [
    `# ${f.name}`,
    '',
    `> ${f.description} MIT licensed, for macOS, Linux and Windows.`,
    '',
    'PolinRider is a supply-chain malware campaign that hides inside software projects, packages and editor extensions. Once it runs on a developer\'s computer it steals passwords and tokens, adds itself to the code they work on and pushes that code with their own account. Security researchers link it to North Korea. The threat page explains it with its sources.',
    '',
    `The program is \`polinrider\`. Latest release: ${f.latestRelease.version} (${f.latestRelease.date}). Install with \`${f.install.homebrew}\` (macOS, Linux) or Scoop on Windows, or download and verify a release. Run it with no arguments; it asks what to check (computer, folder, account, organization or everything).`,
    '',
    'Its answers and exit codes:',
    ...f.verdicts.map((v) => `- ${v.exitCode} ${v.verdict}: ${v.meaning}`),
    '',
    `Content policy: ${policy}`,
    '',
    '## Start here',
    link('The PolinRider threat', f.pages.threat.url, 'what it is, how it spreads, the signs, key facts with sources, questions people ask'),
    link('Quick start', `${SITE_URL}/docs/quick-start/`, 'install, verify and run the scanner'),
    link('Is it PolinRider or a false alarm?', `${SITE_URL}/news/2026-11-04-polinrider-or-false-alarm/`, 'how to read a result'),
    link('Removing PolinRider: do it in this order', `${SITE_URL}/docs/guides/order/`, 'why it comes back, and the order that stops it'),
    '',
    ...groups.flatMap((g) => [`## Docs: ${g}`, ...f.pages.docs.filter((d) => d.group === g).map((d) => link(d.title, d.url, d.description)), '']),
    '## News',
    ...f.pages.news.map((n) => link(n.title, n.url, `${n.date}. ${n.description}`)),
    link('Weekly indicator reviews', f.indicatorSet.reviews, 'every review of the indicator set against published sources'),
    '',
    '## Indicators and releases',
    link('The indicator set', f.indicatorSet.page, 'every indicator with its public source'),
    ...f.indicatorSet.files.map((i) => link(`ioc/${i.file}`, i.url, `${i.meaning} (${i.effect})`)),
    link(`Release ${f.latestRelease.version}`, f.latestRelease.notes),
    link('SHA256SUMS for the latest release', f.latestRelease.checksums),
    link('Verifying a download', f.latestRelease.verifyDocs),
    '',
    '## Machine-readable',
    link('polinrider.json', `${SITE_URL}/data/polinrider.json`, 'latest release, downloads, checksums, verdicts, indicator files and key pages as JSON'),
    link('llms-full.txt', f.feeds.llmsFull, 'the documentation, threat reference and News posts in full, as Markdown'),
    link('RSS', f.feeds.rss, 'News posts and indicator reviews, full text'),
    '',
    '## Optional',
    link('About', f.pages.about.url, 'who maintains it and how releases and indicators are checked'),
    link('Releases', `${SITE_URL}/releases/`),
    link('Source code', f.repository),
    '',
  ].join('\n');
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
