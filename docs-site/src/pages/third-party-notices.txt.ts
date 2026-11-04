// The licence notices of the code this site ships to a browser: Starlight's
// and the Lucode theme's components, scripts and styles, Expressive Code's
// copy button and styles, and Pagefind's search. Each is MIT licensed, and the
// MIT licence asks that its notice travel with copies of the software. This
// page is that copy, served at /polinrider-cleaner/third-party-notices.txt.
// It is read from the installed packages at build time, so it follows every
// upgrade without anyone editing it.
import type { APIRoute } from 'astro';
import { readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const packages = [
  '@astrojs/starlight',
  'lucode-starlight',
  'astro',
  'astro-expressive-code',
  '@expressive-code/core',
  '@expressive-code/plugin-frames',
  '@expressive-code/plugin-text-markers',
  'pagefind',
  '@pagefind/default-ui',
];

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

export const GET: APIRoute = () =>
  new Response(
    'Third-party software in this site\n' +
      '=================================\n\n' +
      'This site is built with the following open source software, parts of which\n' +
      'are served with every page. Their licence notices follow.\n\n' +
      packages.map(notice).join('\n\n\n') +
      '\n',
    { headers: { 'Content-Type': 'text/plain; charset=utf-8' } },
  );
