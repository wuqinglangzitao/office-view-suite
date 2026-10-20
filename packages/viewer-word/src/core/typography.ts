import type { WordTypography } from './types';

// These CJK families have full-em ideographs and half-em Latin spaces in
// Word's balanced-width mode. Browser substitutes can instead use quarter-em
// spaces; measure the selected substitute rather than assuming its metrics.
const CJK_FIXED_FAMILY = /^(宋体|新宋体|仿宋(?:_GB2312)?|黑体|楷体(?:_GB2312)?|simsun|nsimsun|fangsong|simhei|kaiti|ms mincho|ms gothic|mingliu|batang|gulim)$/i;
const OPEN_PUNCTUATION = '（［｛〈《「『【〔〖〘〚';
const CLOSE_PUNCTUATION = '）］｝〉》」』】〕〗〙〛、。，．：；！？';

/** Word's automatic line multiples scale the font line box, not its em size. */
export function layoutWordLineSpacing(root: HTMLElement): void {
  const selector = '[data-word-line-multiple]';
  const paragraphs = root.matches(selector) ? [root] : root.querySelectorAll<HTMLElement>(selector);
  if (!paragraphs.length) return;
  const context = document.createElement('canvas').getContext('2d');
  if (!context) return;
  const metrics = new Map<string, number>();
  const lineHeight = (node: HTMLElement): number => {
    const style = getComputedStyle(node);
    const font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    const text = node.textContent || ' ';
    const key = `${font}\n${text}`;
    let height = metrics.get(key);
    if (height == null) {
      context.font = font;
      const measured = context.measureText(text);
      height = measured.fontBoundingBoxAscent + measured.fontBoundingBoxDescent;
      metrics.set(key, height);
    }
    return height;
  };
  for (const paragraph of paragraphs) {
    const multiple = Number(paragraph.dataset.wordLineMultiple);
    if (!(multiple > 0) || !Number.isFinite(multiple)) continue;
    // Keep each run's metrics: a larger run only expands the lines it occupies.
    // Explicit exact/minimum spacing never enters this path.
    const runs = Array.from(paragraph.querySelectorAll<HTMLElement>('[data-word-run]'));
    const heights = runs.map(lineHeight);
    for (const [index, run] of runs.entries()) {
      const height = heights[index];
      if (height > 0 && Number.isFinite(height)) run.style.lineHeight = `${height * multiple}px`;
    }
    const height = heights.length ? Math.min(...heights) : lineHeight(paragraph);
    if (height > 0 && Number.isFinite(height)) paragraph.style.lineHeight = `${height * multiple}px`;
  }
}

/** Apply parsed document controls before tabs and pagination measure the DOM. */
export function layoutWordTypography(root: HTMLElement, options?: WordTypography): void {
  if (!options) return;
  let context: CanvasRenderingContext2D | null | undefined;
  const spaceWidths = new Map<string, number>();
  for (const run of root.querySelectorAll<HTMLElement>('[data-word-font]')) {
    if (!CJK_FIXED_FAMILY.test(run.dataset.wordFont ?? '')) continue;
    if (options.balanceSingleByteDoubleByteWidth && run.textContent?.includes(' ')) {
      const style = getComputedStyle(run);
      const font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      let width = spaceWidths.get(font);
      if (width == null) {
        context ??= document.createElement('canvas').getContext('2d');
        if (context) { context.font = font; width = context.measureText(' ').width; spaceWidths.set(font, width); }
      }
      if (width != null) run.style.wordSpacing = `${parseFloat(style.fontSize) / 2 - width}px`;
    }
    if (options.compressPunctuation && !run.dataset.wordPunctuation) {
      // Preserve source characters for copying/search. Only the glyph's blank
      // half-em side bearing is compressed, never the character itself.
      for (const node of Array.from(run.childNodes)) {
        if (node.nodeType !== Node.TEXT_NODE) continue;
        const fragment = document.createDocumentFragment();
        for (const part of (node.textContent ?? '').split(/([（［｛〈《「『【〔〖〘〚）］｝〉》」』】〕〗〙〛、。，．：；！？])/)) {
          if (part && (OPEN_PUNCTUATION.includes(part) || CLOSE_PUNCTUATION.includes(part))) {
            const glyph = document.createElement('span');
            glyph.textContent = part;
            glyph.style[OPEN_PUNCTUATION.includes(part) ? 'marginLeft' : 'marginRight'] = '-0.5em';
            fragment.appendChild(glyph);
          } else fragment.appendChild(document.createTextNode(part));
        }
        node.replaceWith(fragment);
      }
      run.dataset.wordPunctuation = 'true';
    }
  }
  if (!options.underlineTrailingSpaces) return;
  for (const group of root.querySelectorAll('.word-centered-space-suffix')) {
    group.replaceWith(...Array.from(group.childNodes));
  }
  for (const run of root.querySelectorAll<HTMLElement>('.u')) {
    if (!/^ +$/.test(run.textContent ?? '')) continue;
    run.classList.add('word-underlined-spaces');
    run.classList.remove('word-hanging-underline');
    run.style.display = 'inline-block';
    run.style.whiteSpace = 'pre';
    run.style.marginRight = '';
    run.style.clipPath = '';
    run.style.removeProperty('--word-underline-width');
  }
  const paragraphs = root.matches('p,li') ? [root] : root.querySelectorAll<HTMLElement>('p,li');
  for (const paragraph of paragraphs) {
    const style = getComputedStyle(paragraph);
    // Right-aligned form fields include the trailing spaces in their width.
    if (style.textAlign === 'right' || style.textAlign === 'end' || style.direction === 'rtl') continue;
    const visit = (node: Node): boolean => {
      if (node.nodeType === Node.TEXT_NODE) return /^ *$/.test(node.textContent ?? '');
      if (!(node instanceof HTMLElement) || !node.childNodes.length) return false;
      if (node.classList.contains('word-underlined-spaces')) {
        // Native hanging spaces retain every source character without forcing
        // a second line, including suffixes split across several source runs.
        node.classList.add('word-hanging-underline');
        node.style.display = 'inline';
        node.style.whiteSpace = 'pre-wrap';
        return true;
      }
      return Array.from(node.childNodes).reverse().every(visit);
    };
    const suffix: Node[] = [];
    Array.from(paragraph.childNodes).reverse().every(node => {
      if (!visit(node)) return false;
      suffix.unshift(node);
      return true;
    });
    if (style.textAlign === 'center' && suffix.length && paragraph.querySelector('.word-hanging-underline')) {
      // Keep centered trailing blanks out of the alignment width. A clipped
      // suffix box prevents hanging spaces from enlarging the table scroller.
      const group = document.createElement('span');
      group.className = 'word-centered-space-suffix';
      group.style.lineHeight = style.lineHeight;
      paragraph.insertBefore(group, suffix[0]);
      group.append(...suffix);
      for (const run of group.querySelectorAll<HTMLElement>('.word-hanging-underline')) run.style.whiteSpace = 'pre';
    }
  }
}

/** Measure decorations after cell indents and tabs establish their positions. */
export function layoutWordUnderlines(root: HTMLElement): void {
  const paragraphs = root.matches('p,li') ? [root] : root.querySelectorAll<HTMLElement>('p,li');
  for (const paragraph of paragraphs) {
    const runs = Array.from(paragraph.querySelectorAll<HTMLElement>('.word-hanging-underline'));
    if (!runs.length) continue;
    for (const run of runs) {
      run.style.marginRight = '';
      run.style.removeProperty('--word-underline-width');
    }
    const bounds = paragraph.getBoundingClientRect();
    const style = getComputedStyle(paragraph);
    const logicalWidth = parseFloat(style.width);
    const scale = logicalWidth > 0 && bounds.width > 0 ? bounds.width / logicalWidth : 1;
    const group = paragraph.querySelector<HTMLElement>(':scope > .word-centered-space-suffix');
    if (group) {
      group.style.width = '0px';
      group.style.marginRight = '0px';
      const width = Math.max(0, bounds.right - group.getBoundingClientRect().left) / scale;
      group.style.width = `${width}px`;
      group.style.marginRight = `${-width}px`;
    }
    for (const run of runs) {
      const rect = run.getBoundingClientRect();
      // Clip only the decoration, preserving the original text for copying.
      const width = Math.max(0, Math.min(rect.width, bounds.right - rect.left)) / scale;
      run.style.setProperty('--word-underline-width', `${width}px`);
    }
  }
}
