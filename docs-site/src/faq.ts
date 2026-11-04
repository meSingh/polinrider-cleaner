// The questions people search for, answered on the page they belong to.
// Each answer is shown on the page (components/Faq.astro) and is also the
// page's FAQPage structured data, so the two can never differ. Answers only
// restate what the page already says, with the same sources.
const base = import.meta.env.BASE_URL.replace(/\/$/, '');
const ext = (href: string, text: string) => `<a href="${href}" target="_blank" rel="noopener noreferrer">${text}</a>`;
const osmMarch = 'https://opensourcemalware.com/blog/polinrider-dprk-compromised-hundreds-of-github-repos';
const osmAb = 'https://opensourcemalware.com/blog/polinrider-is-a-b-testing-its-way-past-your-detections';
const osmHistory = 'https://opensourcemalware.com/blog/how-to-investigate-a-github-repos-real-history';
const thn = 'https://thehackernews.com/2026/07/north-korean-hackers-publish-108.html';

export interface Faq {
  q: string;
  /** HTML: links and code allowed. */
  a: string;
}

export const threatFaq: Faq[] = [
  {
    q: 'Is PolinRider a virus?',
    a: `PolinRider is malware that hides inside software projects, packages and editor extensions. Once it runs on a developer's computer it steals passwords and tokens, adds itself to the code they work on and pushes that code with their own account, so it spreads to the next person who opens the project. Security researchers link it to North Korea (${ext(osmMarch, 'OpenSourceMalware')}; ${ext(thn, 'The Hacker News')}).`,
  },
  {
    q: 'How do I know if PolinRider infected me?',
    a: `Look for the signs on this page: a <code>.woff2</code> font file that holds JavaScript, a <code>.vscode/tasks.json</code> that runs when the folder opens, extra code after the end of a config file, or <code>temp_auto_push.bat</code> in a project. The quickest way to know is to <a href="${base}/docs/quick-start/">run polinrider</a> on your computer and your GitHub account: it answers compromised, review or clean.`,
  },
  {
    q: 'Can I get infected just by opening a project?',
    a: `Yes. A hidden editor task in an infected project runs a command the moment the folder opens in VS Code or Cursor. You do not have to run the project, install anything or click anything (${ext(osmMarch, 'OpenSourceMalware')}; ${ext(osmAb, 'fake fonts')}).`,
  },
  {
    q: 'Does antivirus remove PolinRider?',
    a: `Not reliably. In most variants the payload is decrypted from a blockchain transaction into memory and never written to disk as a program, and the campaign changes its signatures. Windows Defender does flag some variants, such as <code>Trojan:NPM/PolinRider.SF</code>. A detection tells you something ran; a clean scan does not tell you that you are clear. <a href="${base}/news/2026-11-04-polinrider-keeps-coming-back/">Why it keeps coming back</a>.`,
  },
  {
    q: 'Why does my Git history look clean?',
    a: `The script <code>temp_auto_push.bat</code> rewrites your last commit with the malware added, keeping your name, message and the original date, then force-pushes it (${ext(osmMarch, 'OpenSourceMalware')}). <code>git log</code> shows nothing wrong. GitHub's own record of the push keeps the real time and account (${ext(osmHistory, 'OpenSourceMalware')}).`,
  },
  {
    q: 'What does PolinRider steal?',
    a: 'Passwords and logged-in sessions saved in your browser; GitHub tokens, SSH keys and the login of the <code>gh</code> command; npm, cloud, database and CI tokens; secrets in <code>.env</code> files; and cryptocurrency wallets with their recovery phrases. Its goal is not your code: the code is how it travels.',
  },
];

export const falseAlarmFaq: Faq[] = [
  {
    q: 'What does VERDICT: COMPROMISED mean?',
    a: 'polinrider found at least one thing that only the malware leaves. Take a <code>[HIT]</code> at its word: follow the steps under WHAT TO DO NEXT in order, and check this computer before you touch GitHub.',
  },
  {
    q: 'Is a [review] line a sign of infection?',
    a: 'Not on its own. A review line names a file, a process or a setting that a normal project can also have, or says that a check could not run. Ask whether you or your team put it there. If one is not yours, treat it as confirmed.',
  },
  {
    q: 'Does a clean result mean I was never infected?',
    a: 'No. Clean means none of the current indicators is on what was checked. The campaign changes its signatures, so an older variant could have been there and gone. If you had a reason to check, rotate your credentials anyway.',
  },
  {
    q: 'My antivirus flagged Trojan:NPM/PolinRider. Is it a false alarm?',
    a: `No. A detection named for PolinRider, such as Windows Defender's <code>Trojan:NPM/PolinRider.SF</code>, is not a false alarm to wave away: a detection tells you something ran. Check that computer first, then <a href="${base}/docs/guides/order/">clean up in order</a>.`,
  },
];
