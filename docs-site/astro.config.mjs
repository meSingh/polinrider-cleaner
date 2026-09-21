// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import lucode from 'lucode-starlight';

// The site is published to GitHub Pages at /polinrider-cleaner/, so every
// internal link has to be built with that base or the whole thing 404s once
// it leaves localhost.
export default defineConfig({
  site: 'https://mesingh.github.io',
  base: '/polinrider-cleaner',
  trailingSlash: 'always',

  integrations: [
    starlight({
      title: 'polinrider-cleaner',
      description:
        'Detect and clean up after the PolinRider supply-chain campaign, on a machine, a GitHub account, or a whole organization.',

      // Lucode: a shadcn/ui-styled Starlight theme. It supplies the component
      // overrides, tokens and Expressive Code config; everything below is
      // ordinary Starlight configuration on top of it.
      plugins: [
        lucode({
          // Attribution, and where the site came from. Shown on every page.
          footerText:
            'Built by [Mandeep Singh](https://github.com/meSingh) after cleaning up a real ' +
            'PolinRider incident. Source and documentation are [MIT licensed]' +
            '(https://github.com/meSingh/polinrider-cleaner/blob/main/LICENSE). ' +
            'Theme: [Lucode](https://github.com/lucas-labs/lucode-starlight-theme) for ' +
            '[Starlight](https://starlight.astro.build).',
          navLinks: [
            { label: 'Quick start', link: '/quick-start/' },
            { label: 'The campaign', link: '/campaign/what-it-is/' },
            { label: 'Decisions', link: '/project/decisions/', badge: '28' },
          ],
        }),
      ],

      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/meSingh/polinrider-cleaner' },
      ],

      // A correction should be a pull request, not an issue somebody has to
      // translate into one.
      editLink: {
        baseUrl: 'https://github.com/meSingh/polinrider-cleaner/edit/main/docs-site/',
      },

      lastUpdated: true,

      customCss: ['./src/styles/custom.css'],

      sidebar: [
        { label: 'Quick start', link: '/quick-start/' },
        {
          label: 'The campaign',
          items: [
            { label: 'What PolinRider is', link: '/campaign/what-it-is/' },
            { label: 'Why your history looks clean', link: '/campaign/how-it-hides/' },
            { label: 'The indicator set', link: '/campaign/indicators/' },
          ],
        },
        {
          label: 'Cleaning up',
          items: [
            { label: 'Order matters', link: '/guides/order/' },
            { label: 'A machine', link: '/guides/machine/' },
            { label: 'A personal GitHub account', link: '/guides/account/' },
            { label: 'An organization', link: '/guides/organization/' },
            { label: 'Every future push', link: '/guides/ci/' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Exit codes', link: '/reference/exit-codes/' },
            { label: 'False positives you will see', link: '/reference/false-positives/' },
            { label: 'Verifying this repository', link: '/reference/verifying/' },
          ],
        },
        {
          label: 'The project',
          items: [
            { label: 'Why it works this way', link: '/project/decisions/' },
            { label: 'Contributing', link: '/project/contributing/' },
            { label: 'Reporting a vulnerability', link: '/project/security/' },
            { label: 'What this does not promise', link: '/project/disclaimer/' },
          ],
        },
      ],
    }),
  ],
});
