// The licence notices of what this site ships to a browser: Astro's page
// scripts, Pagefind's search (its script, WebAssembly and index format),
// Shiki's code colours, the GitHub mark from Primer Octicons and the Geist
// typeface, then a credit for the generated illustration on What is
// PolinRider?. The code is MIT licensed, and the
// MIT licence asks that its notice travel with copies of the software. Geist
// is under the SIL Open Font License 1.1, which asks the same of its notice
// and licence. This page is that copy, served at
// /third-party-notices.txt under the site base. The code's notices are read
// from the installed packages at build time, so they follow every upgrade
// without anyone editing them. Geist's is the OFL.txt that came with the font
// files, kept beside them in src/assets/fonts/.
import type { APIRoute } from 'astro';
import { readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const packages = ['astro', 'pagefind', 'shiki', '@shikijs/themes'];

const notice = (name: string) => {
  const dir = join(process.cwd(), 'node_modules', name);
  const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
  const file = ['LICENSE', 'LICENSE.md', 'LICENSE.txt', 'license', 'license.md']
    .map((f) => join(dir, f))
    .find((f) => statSync(f, { throwIfNoEntry: false })?.isFile());
  const text = file
    ? readFileSync(file, 'utf8').trim()
    : `Licence: ${pkg.license}, as declared in the package's package.json. The ` +
      'package publishes no licence file or copyright line.';
  return `${name} ${pkg.version}\n${'-'.repeat(name.length + pkg.version.length + 1)}\n\n${text}`;
};

// The GitHub mark in the top bar: mark-github-16 from Primer Octicons, with
// the licence that came with it, kept in src/assets/octicons/.
const octicons = () => {
  const text = readFileSync(join(process.cwd(), 'src/assets/octicons/LICENSE'), 'utf8').trim();
  const head = 'Primer Octicons (the GitHub mark), github.com/primer/octicons';
  return `${head}\n${'-'.repeat(head.length)}\n\n${text}`;
};

// Geist, self-hosted from the vercel/geist-font repository at tag v1.8.1
// (fonts/Geist/webfonts, the two variable-weight files).
const geist = () => {
  const text = readFileSync(join(process.cwd(), 'src/assets/fonts/OFL.txt'), 'utf8').trim();
  const head = 'Geist typeface 1.8.1';
  return `${head}\n${'-'.repeat(head.length)}\n\n${text}`;
};

// Images made with a tool, said plainly.
const images = (() => {
  const head = 'Images';
  return `${head}\n${'-'.repeat(head.length)}\n\nThe illustration on What is PolinRider? was generated with Google Gemini (Nano Banana 2).`;
})();

export const GET: APIRoute = () =>
  new Response(
    'Third-party software in this site\n' +
      '=================================\n\n' +
      'This site is built with the following open source software, parts of which\n' +
      'are served with every page. Their licence notices follow.\n\n' +
      [...packages.map(notice), geist(), octicons()].join('\n\n\n') +
      '\n\n\n' +
      images +
      '\n',
    { headers: { 'Content-Type': 'text/plain; charset=utf-8' } },
  );
