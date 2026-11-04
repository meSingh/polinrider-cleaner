// What an Updates article leaves out of its rendered text: the "Check yours"
// box older articles end with, and the heading of a questions section (the
// questions themselves are not drawn inside an article). Used by the
// article layout and the RSS feed, so both show the same text.
export const plainArticle = (html: string) =>
  html
    .replace(/<aside class="aside aside-tip" aria-label="Check yours">[\s\S]*?<\/aside>/g, '')
    .replace(/<h2[^>]*id="questions-people-ask"[^>]*>[\s\S]*?<\/h2>/g, '');
