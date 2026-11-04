// The site's content, one collection per section. Each file is a page; its
// address comes from its path (dots kept, so releases/v2.3.0.mdx is
// /releases/v2.3.0/). Unknown front matter, such as the `sidebar` and `slug`
// fields older pages carry, is ignored rather than refused.
import { defineCollection } from 'astro:content';
import { file, glob } from 'astro/loaders';
import { z } from 'astro/zod';

const byPath = ({ entry }: { entry: string }) => entry.replace(/\.mdx?$/, '');
const pages = (base: string, pattern: string | string[] = '**/*.{md,mdx}') =>
  glob({ pattern, base, generateId: byPath });

// YAML reads an unquoted date as a Date; accept either and keep YYYY-MM-DD.
const day = z
  .union([z.string(), z.date()])
  .transform((d) => (d instanceof Date ? d.toISOString().slice(0, 10) : d));

const page = z.object({
  title: z.string(),
  description: z.string().optional(),
  // A shorter name for lists and the sidebar, where the title is too long.
  sidebar: z.object({ label: z.string().optional() }).optional(),
  // A page written ahead of a fact it depends on: kept in the repository,
  // never built, until the line is removed.
  draft: z.boolean().optional(),
});

export const collections = {
  // The documentation for the latest release, served at /docs/. The weekly
  // indicator reviews live in the same folder, because ioc/ cites each one
  // by that path, but are their own collection, served under /news/.
  docs: defineCollection({
    loader: pages('./src/content/docs', ['**/*.{md,mdx}', '!indicator-reviews/**']),
    schema: page,
  }),
  // Frozen copies, one folder per release line, served at /docs/X.Y/.
  // Written by scripts/freeze-docs.mjs at a release, never by hand.
  docsVersions: defineCollection({ loader: pages('./src/content/docs-versions'), schema: page }),
  // The threat page (index.mdx) and the full indicator set.
  threat: defineCollection({ loader: pages('./src/content/threat'), schema: page }),
  news: defineCollection({
    loader: pages('./src/content/news'),
    schema: page.extend({ description: z.string(), date: day, review: day.optional() }),
  }),
  reviews: defineCollection({ loader: pages('./src/content/docs/indicator-reviews'), schema: page }),
  releases: defineCollection({
    loader: pages('./src/content/releases'),
    schema: page.extend({
      description: z.string(),
      version: z.coerce.string(),
      date: day,
      breaking: z.string().optional(),
    }),
  }),
  // Articles about the campaign published elsewhere, each one read and
  // checked before it was added. See the file's header.
  elsewhere: defineCollection({
    loader: file('./src/content/elsewhere.yaml'),
    schema: z.object({
      url: z.url(),
      source: z.string(),
      title: z.string(),
      date: day,
      summary: z.string(),
    }),
  }),
};
