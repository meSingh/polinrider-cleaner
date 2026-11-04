// robots.txt, built with the site's own address. Everything here is public
// and meant to be found: search engines and AI assistants alike may read,
// index and cite it. Crawlers read robots.txt only at a host's root, so it
// takes effect once the site is served from its own domain (ON_OWN_DOMAIN).
import type { APIRoute } from 'astro';
import { SITE_URL } from '../siteConfig.mjs';

// AI search, assistants and the crawlers behind them, named so the policy is
// explicit rather than implied by the wildcard. All are allowed.
const ai = [
  'GPTBot',
  'OAI-SearchBot',
  'ChatGPT-User',
  'ClaudeBot',
  'Claude-SearchBot',
  'Claude-User',
  'Claude-Web',
  'anthropic-ai',
  'PerplexityBot',
  'Perplexity-User',
  'Google-Extended',
  'Applebot-Extended',
  'CCBot',
  'Bytespider',
  'meta-externalagent',
  'Amazonbot',
  'DuckAssistBot',
  'MistralAI-User',
  'cohere-ai',
];

export const GET: APIRoute = () =>
  new Response(
    [
      '# PolinRider Scanner and Cleaner. Everything on this site is public and may',
      '# be read, indexed and cited, by search engines and AI assistants alike.',
      `# A summary for language models: ${SITE_URL}/llms.txt`,
      '',
      'User-agent: *',
      'Allow: /',
      '',
      ...ai.map((a) => `User-agent: ${a}`),
      'Allow: /',
      '',
      `Sitemap: ${SITE_URL}/sitemap-index.xml`,
      '',
    ].join('\n'),
    { headers: { 'Content-Type': 'text/plain; charset=utf-8' } },
  );
