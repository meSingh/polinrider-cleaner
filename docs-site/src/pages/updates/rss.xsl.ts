// How the Updates feed looks when someone opens it in a browser instead of a
// feed reader: a plain page that says what a feed is, with every item's
// title, date, category and summary. Browsers that still apply XSLT use it;
// the rest show the feed as before. Readers ignore it.
import type { APIRoute } from 'astro';

const xsl = `<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="html" encoding="UTF-8" indent="yes"/>
  <xsl:template match="/rss/channel">
    <html lang="en-IN">
      <head>
        <meta charset="utf-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <meta name="robots" content="noindex"/>
        <title><xsl:value-of select="title"/> (feed)</title>
        <style>
          :root { color-scheme: light dark; --ink: #0f1220; --text: #474c59; --muted: #5b6170; --line: #dfe3ea; --accent: #0b6a80; --tint: #ecf7f9; --bg: #fff; }
          @media (prefers-color-scheme: dark) { :root { --ink: #f4f6fa; --text: #c3c9d4; --muted: #9aa3b2; --line: #262c38; --accent: #7adcea; --tint: rgb(122 220 234 / 0.1); --bg: #0c0f15; } }
          body { margin: 0; background: var(--bg); color: var(--text); font: 16px/1.6 system-ui, -apple-system, "Segoe UI", sans-serif; }
          main { max-width: 46rem; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; }
          h1 { margin: 0; color: var(--ink); font-size: 1.75rem; line-height: 1.2; }
          .about { margin: 1rem 0 2rem; padding: 0.9rem 1.1rem; border-inline-start: 3px solid var(--accent); border-radius: 8px; background: var(--tint); }
          .about code { font-size: 0.875em; word-break: break-all; }
          ol { margin: 0; padding: 0; list-style: none; }
          li { padding: 1rem 0; border-bottom: 1px solid var(--line); }
          .meta { margin: 0; color: var(--muted); font-size: 0.8125rem; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; }
          h2 { margin: 0.3rem 0 0; font-size: 1.125rem; line-height: 1.35; }
          h2 a { color: var(--ink); text-decoration: none; }
          h2 a:hover { text-decoration: underline; }
          p.sum { margin: 0.35rem 0 0; }
          a { color: var(--accent); }
        </style>
      </head>
      <body>
        <main>
          <h1><xsl:value-of select="title"/></h1>
          <p class="about">
            This is a feed. Copy its address into a feed reader to get every update and weekly indicator review as it is
            published: <code><xsl:value-of select="*[local-name()='link'][@rel='self']/@href"/></code>.
            Or read them on <a href="{link}">the Updates page</a>.
          </p>
          <ol>
            <xsl:for-each select="item">
              <li>
                <p class="meta"><xsl:value-of select="category"/> · <xsl:value-of select="substring(pubDate, 6, 11)"/></p>
                <h2><a href="{link}"><xsl:value-of select="title"/></a></h2>
                <p class="sum"><xsl:value-of select="description"/></p>
              </li>
            </xsl:for-each>
          </ol>
        </main>
      </body>
    </html>
  </xsl:template>
</xsl:stylesheet>
`;

export const GET: APIRoute = () => new Response(xsl, { headers: { 'Content-Type': 'text/xsl; charset=utf-8' } });
