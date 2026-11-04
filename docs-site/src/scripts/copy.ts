// A Copy button on every code block in a Markdown page and on the run block.
// Copies the text exactly as shown. Without JavaScript there is no button,
// and the text can still be selected by hand.
const label = (btn: HTMLButtonElement, text: string) => {
  btn.querySelector('span')!.textContent = text;
};

for (const pre of document.querySelectorAll<HTMLPreElement>('.prose pre, .code-block pre')) {
  let wrap = pre.parentElement!;
  if (!wrap.classList.contains('copy-wrap')) {
    wrap = document.createElement('div');
    wrap.className = 'copy-wrap';
    pre.replaceWith(wrap);
    wrap.append(pre);
  }
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'copy-btn';
  btn.innerHTML = '<span>Copy</span>';
  btn.setAttribute('aria-label', 'Copy to clipboard');
  btn.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(pre.innerText.replace(/\n$/, ''));
      btn.dataset.copied = '';
      label(btn, 'Copied');
    } catch {
      label(btn, 'Select and copy');
    }
    setTimeout(() => {
      delete btn.dataset.copied;
      label(btn, 'Copy');
    }, 2000);
  });
  wrap.append(btn);
}
