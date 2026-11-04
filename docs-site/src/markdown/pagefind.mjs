// Site search. Pagefind reads the built HTML once the build is done and
// writes a static index to dist/pagefind/. Searching then happens in the
// reader's browser against those files: nothing is sent anywhere. Only the
// part of a page marked data-pagefind-body is indexed, which the layouts set
// on the docs, the campaign pages, Updates and Releases (and not on frozen
// copies of the docs, which would repeat every result).
import { fileURLToPath } from 'node:url';
import * as pagefind from 'pagefind';

const check = (response) => {
  if (response.errors.length) throw new Error(`Pagefind: ${response.errors.join('; ')}`);
  return response;
};

export function pagefindIndex() {
  return {
    name: 'polinrider-pagefind',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        try {
          const { index } = check(await pagefind.createIndex());
          const { page_count } = check(await index.addDirectory({ path: fileURLToPath(dir) }));
          check(await index.writeFiles({ outputPath: fileURLToPath(new URL('./pagefind/', dir)) }));
          logger.info(`Search index built from ${page_count} pages.`);
        } finally {
          await pagefind.close();
        }
      },
    },
  };
}
