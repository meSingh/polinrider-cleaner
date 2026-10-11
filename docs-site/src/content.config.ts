import { defineCollection } from 'astro:content';
import { docsLoader, i18nLoader } from '@astrojs/starlight/loaders';
import { docsSchema, i18nSchema } from '@astrojs/starlight/schema';
import { ExtendDocsSchema } from 'lucode-starlight/schema';

// ExtendDocsSchema is required by the Lucode theme: without it Starlight
// strips the theme's own hero frontmatter (layout, announcement, button
// styles) before the theme ever sees it.
export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({ extend: ExtendDocsSchema }),
  }),
  // Declared but empty. The theme reads this collection, and without it every
  // dev-server start logs "the collection i18n does not exist or is empty".
  // The site is English-only; this exists to stop a warning that is not one.
  i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema() }),
};
