// What the threat page and its social image share: the sources it cites and
// the four key figures, each with its source. Every figure keeps its
// qualifier ("about"). statChips are the same figures, shorter, for the image.
export const src = {
  osmMarch: 'https://opensourcemalware.com/blog/polinrider-dprk-compromised-hundreds-of-github-repos',
  osmBlast: 'https://opensourcemalware.com/blog/polinrider-blast-radius-grows',
  osmAb: 'https://opensourcemalware.com/blog/polinrider-is-a-b-testing-its-way-past-your-detections',
  osmHistory: 'https://opensourcemalware.com/blog/how-to-investigate-a-github-repos-real-history',
  osm2025: 'https://opensourcemalware.com/blog/polinrider-malware-found-in-2025-npm-packages',
  osmTasks: 'https://opensourcemalware.com/blog/tasksjacker-dprk-attack-github-vscode',
  osmApril: 'https://opensourcemalware.com/blog/polinrider-rides-again-north-korean-attack-expands-across-github',
  thn: 'https://thehackernews.com/2026/07/north-korean-hackers-publish-108.html',
  safedepEth: 'https://safedep.io/polinrider-ethereum-c2-github-repositories/',
  safedepMs64: 'https://safedep.io/microsoftsystem64-binary-payload-analysis/',
  dp: 'https://blog.deception.pro/blog/hok-dprk-polinrider-sep-2026',
};

export const stats = [
  { figure: 'About 4,367', what: 'infected repositories, across 2,152 owners', source: 'OpenSourceMalware, 15 July 2026', href: src.osmBlast },
  { figure: '108', what: 'malicious packages and extensions across npm, Packagist, Go and Chrome', source: 'The Hacker News, 4 July 2026', href: src.thn },
  { figure: '35', what: 'GitHub repositories with a loader that reads its server addresses from Ethereum', source: 'SafeDep, 30 September 2026', href: src.safedepEth },
  { figure: 'May 2025', what: 'the earliest known PolinRider code, found later in nine npm versions', source: 'OpenSourceMalware, 30 October 2026', href: src.osm2025 },
];

/** The figures as they read on the social image, shorter. */
export const statChips = [
  { figure: 'About 4,367', label: 'infected repositories' },
  { figure: '108', label: 'malicious packages and extensions' },
  { figure: '35', label: 'repositories using Ethereum for C2' },
  { figure: 'May 2025', label: 'earliest known code' },
];

/** The quotable one-sentence answer at the top of the page. */
export const answer =
  'PolinRider is malware that hides inside software projects, steals developers\u2019 logins and spreads by pushing infected code with their own accounts.';
