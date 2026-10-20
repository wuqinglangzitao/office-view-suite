/** Split rendered flow content without discarding inline formatting or text. */
export function firstFlowLinesBottom(node: HTMLElement, count: number): number {
  const rectangles: DOMRect[] = [];
  const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const range = document.createRange();
    range.selectNodeContents(walker.currentNode);
    rectangles.push(...Array.from(range.getClientRects()).filter(rect => rect.width > 0 && rect.height > 0));
  }
  rectangles.sort((left, right) => left.top - right.top);
  const lines: { top: number; bottom: number }[] = [];
  for (const rect of rectangles) {
    const last = lines[lines.length - 1];
    if (last && rect.top < last.bottom - 0.1) last.bottom = Math.max(last.bottom, rect.bottom);
    else lines.push({ top: rect.top, bottom: rect.bottom });
  }
  if (!lines.length) return node.getBoundingClientRect().bottom;
  const line = lines[Math.min(count, lines.length) - 1];
  const lineHeight = Number.parseFloat(getComputedStyle(node).lineHeight) || 0;
  return Math.max(line.bottom, node.getBoundingClientRect().top + Math.min(count, lines.length) * lineHeight);
}

export function splitFlowNode(
  source: HTMLElement,
  fits: (node: HTMLElement) => boolean,
  hasPrevious: boolean,
): { head: HTMLElement; tail: HTMLElement } | undefined {
  if (!source.matches('p, h1, h2, h3, h4, h5, h6, .list-run')) return;
  const items = Array.from(source.querySelectorAll<HTMLElement>('li'));
  let itemIndex = items.findIndex(item => !fits(item));
  if (itemIndex < 0) itemIndex = items.length - 1;
  // Prefer complete list items, including a keep-with-next chain, on each page.
  let boundary = itemIndex;
  while (boundary > 0 && items[boundary - 1].dataset.keepNext === 'true') boundary -= 1;
  for (; boundary > 0; boundary -= 1) {
    const range = document.createRange();
    range.selectNodeContents(source);
    range.setEndBefore(items[boundary]);
    const pair = fragments(source, range);
    if (measure(source, pair.head, fits)) return pair;
  }

  const paragraph = items[itemIndex] ?? source;
  if (hasPrevious && paragraph.dataset.keepLines === 'true') return;
  const textNodes: Text[] = [];
  const walker = document.createTreeWalker(source, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) textNodes.push(walker.currentNode as Text);
  const length = textNodes.reduce((sum, node) => sum + node.length, 0);
  if (length < 2) return;
  const rangeAt = (offset: number): Range => {
    const range = document.createRange();
    range.selectNodeContents(source);
    for (const text of textNodes) {
      if (offset <= text.length) {
        range.setEnd(text, offset);
        return range;
      }
      offset -= text.length;
    }
    return range;
  };
  let low = 0;
  let high = length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    const { head } = fragments(source, rangeAt(middle));
    if (measure(source, head, fits)) low = middle;
    else high = middle - 1;
  }
  if (low === 0) return;

  const text = source.textContent ?? '';
  // Keep surrogate pairs, combining marks and other grapheme clusters intact.
  let offset = graphemeBoundary(text, low);
  // An English word that fits a full line should travel with the following text.
  const wordStart = text.slice(0, offset).search(/[A-Za-z0-9]+$/);
  if (wordStart > 0 && /[A-Za-z0-9]/.test(text[offset] ?? '')) {
    const trial = fragments(source, rangeAt(wordStart));
    if (lineCount(trial.head, source) > 0) offset = wordStart;
  }
  if (offset === 0) return;
  let pair = fragments(source, rangeAt(offset));
  // Word normally keeps at least two lines of a paragraph on either page.
  if (paragraph.dataset.widowControl !== 'false') {
    const heads = lineCount(pair.head, source);
    if (heads < 2 && hasPrevious) return;
    if (lineCount(pair.tail, source) < 2 && heads > 2) {
      const targetLines = heads - 1;
      let start = 1;
      let end = offset;
      while (start < end) {
        const middle = Math.ceil((start + end) / 2);
        const candidate = fragments(source, rangeAt(middle));
        if (lineCount(candidate.head, source) <= targetLines) start = middle;
        else end = middle - 1;
      }
      pair = fragments(source, rangeAt(graphemeBoundary(text, start)));
    }
  }
  if (!hasFlow(pair.head) || !hasFlow(pair.tail) || !measure(source, pair.head, fits)) return;
  return pair;
}

function fragments(source: HTMLElement, prefix: Range): { head: HTMLElement; tail: HTMLElement } {
  const suffix = document.createRange();
  suffix.selectNodeContents(source);
  suffix.setStart(prefix.endContainer, prefix.endOffset);
  const head = source.cloneNode(false) as HTMLElement;
  const tail = source.cloneNode(false) as HTMLElement;
  head.appendChild(prefix.cloneContents());
  tail.appendChild(suffix.cloneContents());
  head.style.marginBottom = '0';
  tail.style.marginTop = '0';
  for (const node of [head, tail]) {
    for (const list of Array.from(node.querySelectorAll('ol, ul')).reverse()) {
      if (!list.childNodes.length) list.remove();
    }
  }
  const startElement = prefix.endContainer.nodeType === Node.TEXT_NODE
    ? prefix.endContainer.parentElement : prefix.endContainer as Element;
  const continuedItem = startElement?.closest('li');
  // A split inside an item keeps its indentation but prints its marker once.
  if (continuedItem) {
    const item = tail.querySelector<HTMLElement>('li');
    if (item) {
      item.dataset.continuation = 'true';
      item.style.textIndent = '0';
      item.style.marginTop = '0';
    }
  }
  if (!source.classList.contains('list-run')) tail.style.textIndent = '0';
  return { head, tail };
}

function measure(source: HTMLElement, candidate: HTMLElement, check: (node: HTMLElement) => boolean): boolean {
  source.replaceWith(candidate);
  try { return check(candidate); }
  finally { candidate.replaceWith(source); }
}

function lineCount(candidate: HTMLElement, source: HTMLElement): number {
  source.replaceWith(candidate);
  try {
    const lines: number[] = [];
    const walker = document.createTreeWalker(candidate, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const range = document.createRange();
      range.selectNodeContents(walker.currentNode);
      for (const rect of range.getClientRects()) {
        if (rect.width > 0 && rect.height > 0 && !lines.some(top => Math.abs(top - rect.top) < rect.height * 0.35)) {
          lines.push(rect.top);
        }
      }
    }
    return lines.length;
  } finally { candidate.replaceWith(source); }
}

function hasFlow(node: HTMLElement): boolean {
  return !!node.textContent?.length || !!node.querySelector('img');
}

function graphemeBoundary(text: string, limit: number): number {
  let boundary = 0;
  for (const segment of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) {
    const end = segment.index + segment.segment.length;
    if (end > limit) break;
    boundary = end;
  }
  return boundary;
}
