// The four callouts the pages use, written in Markdown as
//
//   :::caution[Optional title]
//   Text.
//   :::
//
// and rendered as <aside class="aside aside-caution">. Note and tip take the
// accent colour; caution and danger take the review and compromised colours,
// with an icon drawn in CSS, so colour is never the only signal. A container
// with any other name is kept as a plain <div> so no text is ever dropped.
// A Satteri mdast plugin (Astro 7's Markdown processor), registered in
// astro.config.mjs for both .md and .mdx.

const VARIANTS = { note: 'Note', tip: 'Tip', caution: 'Caution', danger: 'Danger' };

const text = (node) =>
  node.value ?? (node.children ?? []).map(text).join('');

const el = (tagName, properties, children = []) => ({
  type: 'paragraph',
  data: { hName: tagName, hProperties: properties },
  children,
});

export function asides() {
  return {
    name: 'polinrider-asides',
    containerDirective(node) {
      const children = [...node.children];
      if (!(node.name in VARIANTS)) return el('div', {}, children);

      let titleNodes = [{ type: 'text', value: VARIANTS[node.name] }];
      let title = VARIANTS[node.name];
      const first = children[0];
      if (first?.type === 'paragraph' && first.data?.directiveLabel && first.children.length) {
        titleNodes = first.children;
        title = text(first);
        children.shift();
      }
      return el('aside', { class: `aside aside-${node.name}`, 'aria-label': title }, [
        el('p', { class: 'aside-title', 'aria-hidden': 'true' }, titleNodes),
        el('div', { class: 'aside-body' }, children),
      ]);
    },
  };
}
