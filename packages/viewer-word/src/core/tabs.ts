import type { WordTabStop } from './types';

const PT_TO_PX = 4 / 3;
const LEADERS: Partial<Record<WordTabStop['leader'], string>> = {
  dot: '.', middleDot: '·', hyphen: '-', underscore: '_', heavy: '_',
};

/** Keep the source tab in textContent; draw its leader without adding document text. */
export function appendTabbedText(parent: HTMLElement, text: string): void {
  for (const [index, part] of text.split('\t').entries()) {
    if (index) {
      const tab = document.createElement('span');
      tab.className = 'word-tab';
      const source = document.createElement('span');
      source.className = 'word-tab-source';
      source.textContent = '\t';
      tab.appendChild(source);
      parent.appendChild(tab);
    }
    if (part) parent.appendChild(document.createTextNode(part));
  }
}

/** Measure only after attachment, inside the renderer's unscaled pagination pass. */
export function layoutWordTabs(root: HTMLElement, defaultStopPt = 36): void {
  const paragraphs = root.matches('[data-word-tabs]') ? [root]
    : Array.from(root.querySelectorAll<HTMLElement>('[data-word-tabs]'));
  for (const paragraph of paragraphs) {
    const tabs = Array.from(paragraph.querySelectorAll<HTMLElement>('.word-tab'));
    if (!tabs.length) continue;
    const stops: WordTabStop[] = JSON.parse(paragraph.dataset.wordTabs ?? '[]');
    const interval = (defaultStopPt > 0 ? defaultStopPt : 36) * PT_TO_PX;
    for (const tab of tabs) { tab.style.width = '0px'; delete tab.dataset.leader; delete tab.dataset.leaderText; }
    const style = getComputedStyle(paragraph);
    // Source stop positions are relative to the text area, before paragraph indents.
    const origin = paragraph.getBoundingClientRect().left - (parseFloat(style.marginLeft) || 0)
      + (parseFloat(style.paddingLeft) || 0);
    for (const [index, tab] of tabs.entries()) {
      const current = tab.getBoundingClientRect().left - origin;
      const stop = stops.find(stop => stop.alignment !== 'clear' && stop.alignment !== 'bar'
        && stop.positionPt * PT_TO_PX > current + 0.25);
      let target = stop ? stop.positionPt * PT_TO_PX : (Math.floor((current + 0.25) / interval) + 1) * interval;
      if (stop && stop.alignment !== 'left') {
        const field = followingField(paragraph, tab, tabs[index + 1]);
        if (stop.alignment === 'decimal') trimAtDecimal(field, paragraph);
        const width = measureField(paragraph, field);
        // CSS zoom can round each glyph to a different layout unit after the
        // unscaled pass. Leave a subpixel allowance at an aligned endpoint so
        // a number touching the right margin does not jump to another line.
        target -= (stop.alignment === 'center' ? width / 2 : width) + 0.125;
      }
      const width = Math.max(0, target - current);
      tab.style.width = `${width}px`;
      tab.dataset.leader = stop?.leader ?? 'none';
      const leader = stop && LEADERS[stop.leader];
      if (leader && width > 0) {
        // CSS clips to the tab width; the repeated decoration stays out of textContent.
        const fontSize = parseFloat(getComputedStyle(tab).fontSize) || 12;
        tab.dataset.leaderText = leader.repeat(Math.min(4096, Math.ceil(width / (fontSize * 0.15)) + 2));
      }
    }
  }
}

function followingField(paragraph: HTMLElement, tab: HTMLElement, next?: HTMLElement): Range {
  const range = document.createRange();
  range.setStartAfter(tab);
  if (next) range.setEndBefore(next);
  else range.setEnd(paragraph, paragraph.childNodes.length);
  const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
  let node: Node | null;
  while ((node = walker.nextNode())) {
    if (!range.isPointInRange(node, 0)) continue;
    const lineBreak = node.textContent?.indexOf('\n') ?? -1;
    if (lineBreak >= 0) { range.setEnd(node, lineBreak); break; }
  }
  return range;
}

function trimAtDecimal(range: Range, paragraph: HTMLElement): void {
  const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
  let node: Node | null;
  while ((node = walker.nextNode())) {
    if (!range.isPointInRange(node, 0)) continue;
    const decimal = node.textContent?.search(/[.,]/) ?? -1;
    if (decimal >= 0) { range.setEnd(node, decimal); break; }
  }
}

function measureField(paragraph: HTMLElement, range: Range): number {
  const probe = paragraph.cloneNode(false) as HTMLElement;
  probe.removeAttribute('id');
  probe.removeAttribute('data-word-tabs');
  if (probe.tagName === 'LI') probe.dataset.continuation = 'true';
  Object.assign(probe.style, {
    position: 'absolute', visibility: 'hidden', width: 'max-content', maxWidth: 'none',
    margin: '0', padding: '0', border: '0', textIndent: '0', whiteSpace: 'pre',
  });
  probe.appendChild(range.cloneContents());
  paragraph.parentElement!.appendChild(probe);
  const width = probe.getBoundingClientRect().width;
  probe.remove();
  return width;
}
