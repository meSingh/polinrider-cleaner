// Questions written for the false-alarm article, which still imports them.
// Articles show no questions section (layouts/Update.astro) and carry no
// FAQPage data, so these are not published anywhere today. Kept only so
// the article builds until its rewrite drops the import.
const base = import.meta.env.BASE_URL.replace(/\/$/, '');

export interface Faq {
  q: string;
  /** HTML: links and code allowed. */
  a: string;
}

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
