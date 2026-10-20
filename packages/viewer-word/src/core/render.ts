import { SHADOW_CSS } from './styles';
import { firstFlowLinesBottom, splitFlowNode } from './pagination';
import { appendTabbedText, layoutWordTabs } from './tabs';
import { renderWordChart } from './charts';
import { layoutWordLineSpacing, layoutWordTypography, layoutWordUnderlines } from './typography';
import type { WordBlock, WordCell, WordDoc, WordInline, WordSectionSetup, WordTable } from './types';

const FIRST_CHUNK = 120;
const IDLE_CHUNK = 80;
const IDLE_BUDGET_MS = 50;
let tableSequence = 0;

/** Word 高亮色名 → CSS */
const HIGHLIGHT: Record<string, string> = {
  yellow: '#ffeb3b', green: '#80ff80', cyan: '#80ffff', magenta: '#ff80ff',
  blue: '#99c2ff', red: '#ff9999', darkBlue: '#6699cc', darkCyan: '#66cccc',
  darkGreen: '#66cc66', darkMagenta: '#cc66cc', darkRed: '#cc6666',
  darkYellow: '#cccc66', darkGray: '#a0a0a0', lightGray: '#d9d9d9',
  black: '#000000', white: '#ffffff', none: 'transparent',
};

type RenderUnit = { kind: 'single'; block: WordBlock } | { kind: 'listRun'; items: WordBlock[] };
type SectionRun = { setup: WordSectionSetup; units: RenderUnit[] };
type PageState = { body: HTMLElement; page: HTMLElement; columns: HTMLElement[]; column: number; setup: WordSectionSetup; sectionPage: number };

/**
 * 渲染入口:Shadow DOM 隔离 + 渲染单元分批挂载。
 * 字体/字号/颜色/高亮/对齐/缩进/行距全部映射为内联样式。
 */
export function renderWordInto(host: HTMLElement, doc: WordDoc): {
  stats: { blocks: number; paragraphs: number; tables: number; images: number };
  resize(): void;
  getContentWidth(): number;
  dispose(): void;
} {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  shadow.replaceChildren();

  const style = document.createElement('style');
  style.textContent = SHADOW_CSS;
  shadow.appendChild(style);

  const container = document.createElement('div');
  container.className = 'doc';
  shadow.appendChild(container);
  const layoutText = (root: HTMLElement) => {
    layoutWordLineSpacing(root);
    layoutWordTypography(root, doc.typography);
    layoutTableCellIndents(root);
    layoutWordTabs(root, doc.defaultTabStopPt);
    layoutWordUnderlines(root);
  };

  const sections = splitSections(doc);
  const pageWidth = sections.reduce((width, section) => Math.max(width, section.setup.page.widthPt * 4 / 3), 0);
  let sectionIndex = 0;
  let unitIndex = 0;
  let globalPage = 0;
  let current: PageState | undefined;
  let sectionStarted = false;
  let disposed = false;
  let cancelPending: (() => void) | undefined;
  let reflowFrame: number | undefined;
  const imageSizes = new Map<string, { width: number; height: number }>();

  const prepareImages = (node: HTMLElement) => {
    for (const image of node.querySelectorAll('img')) {
      const size = imageSizes.get(image.src);
      if (!size) continue;
      if (!image.hasAttribute('width')) image.width = image.height ? image.height * size.width / size.height : size.width;
      if (!image.hasAttribute('height')) image.height = image.width * size.height / size.width;
    }
  };

  const newPage = (setup: WordSectionSetup, sectionPage: number): PageState => {
    globalPage += 1;
    const page = document.createElement('section');
    page.className = 'page';
    page.style.width = `${setup.page.widthPt}pt`;
    page.style.height = `${setup.page.heightPt}pt`;
    page.dataset.section = String(sectionIndex);
    page.dataset.sectionPage = String(sectionPage);
    const previous = container.querySelector<HTMLElement>('.page:last-child');
    const displayNumber = sectionPage === 1 && setup.pageNumberStart != null ? setup.pageNumberStart
      : Number(previous?.dataset.pageNumber ?? 0) + 1;
    page.dataset.pageNumber = String(displayNumber);
    page.dataset.pageFormat = setup.pageNumberFormat ?? 'decimal';

    const header = document.createElement('header');
    header.className = 'page-header';
    header.style.left = `${setup.page.leftPt}pt`;
    header.style.right = `${setup.page.rightPt}pt`;
    header.style.top = `${setup.headerDistancePt ?? Math.max(6, setup.page.topPt * 0.2)}pt`;
    appendRunningBlocks(header, runningBlocks(setup, sectionPage, true));

    const body = document.createElement('main');
    body.className = 'page-body';
    body.style.left = `${setup.page.leftPt}pt`;
    body.style.right = `${setup.page.rightPt}pt`;
    body.style.top = `${setup.page.topPt}pt`;
    body.style.bottom = `${setup.page.bottomPt}pt`;
    if (setup.linePitchPt) body.style.lineHeight = `${setup.linePitchPt}pt`;
    const columnCount = Math.max(1, Math.min(64, setup.columnWidthsPt?.length || setup.columns || 1));
    const columns: HTMLElement[] = [];
    const writingMode = sectionWritingMode(setup.textDirection);
    if (writingMode) body.style.writingMode = writingMode;
    if (columnCount > 1) {
      body.style.display = 'flex';
      body.style.flexDirection = setup.columnsRtl ? 'row-reverse' : 'row';
      for (let index = 0; index < columnCount; index += 1) {
        const column = document.createElement('div');
        column.className = 'page-column';
        column.style.position = 'relative';
        column.style.minWidth = '0';
        column.style.height = '100%';
        column.style.flex = setup.columnWidthsPt?.[index] ? `0 0 ${setup.columnWidthsPt[index]}pt` : '1 1 0';
        if (index < columnCount - 1) {
          const gap = setup.columnSpacesPt?.[index] ?? setup.columnGapPt ?? 36;
          column.style.marginInlineEnd = `${gap}pt`;
          if (setup.columnSeparator) {
            column.dataset.separator = 'true';
            column.style.setProperty('--separator-offset', `${-gap / 2}pt`);
          }
        }
        body.appendChild(column);
        columns.push(column);
      }
    } else columns.push(body);

    const footer = document.createElement('footer');
    footer.className = 'page-footer';
    footer.style.left = `${setup.page.leftPt}pt`;
    footer.style.right = `${setup.page.rightPt}pt`;
    footer.style.bottom = `${setup.footerDistancePt ?? Math.max(6, setup.page.bottomPt * 0.2)}pt`;
    const footerBlocks = runningBlocks(setup, sectionPage, false);
    appendRunningBlocks(footer, footerBlocks);
    if (setup.pageNumbers && footerBlocks.length === 0) {
      const number = document.createElement('span');
      number.className = 'page-number';
      number.textContent = formatPageNumber(displayNumber, setup.pageNumberFormat);
      footer.appendChild(number);
    }

    page.append(header, body, footer);
    container.appendChild(page);
    layoutRunningImages(header, page, body);
    layoutRunningImages(footer, page, body);
    layoutText(header);
    layoutText(footer);
    updatePageFields(page, globalPage, sectionPage);
    return { body: columns[0], page, columns, column: 0, setup, sectionPage };
  };

  const ensurePage = (setup: WordSectionSetup): PageState => {
    current ??= newPage(setup, 1);
    return current;
  };

  const advancePage = (setup: WordSectionSetup): PageState => {
    if (current && current.column + 1 < current.columns.length) {
      current.column += 1;
      current.body = current.columns[current.column];
      return current;
    }
    const next = (current?.sectionPage ?? 0) + 1;
    current = newPage(setup, next);
    return current;
  };

  const pageBounds = (page: PageState): { bottom: number; scale: number } => {
    const rect = page.body.getBoundingClientRect();
    const logicalHeight = Number.parseFloat(getComputedStyle(page.body).height);
    // Rectangles include CSS zoom while computed margins do not. Measure the
    // effective scale without offsetHeight's integer rounding at small zooms.
    const scale = logicalHeight > 0 && rect.height > 0 ? rect.height / logicalHeight : 1;
    return { bottom: rect.bottom + 0.5 * scale, scale };
  };

  const layoutBottom = (node: HTMLElement, scale: number): number => {
    const marginBottom = Number.parseFloat(getComputedStyle(node).marginBottom) || 0;
    return node.getBoundingClientRect().bottom + marginBottom * scale;
  };

  const fitsPage = (node: HTMLElement, page: PageState): boolean => {
    const mode = getComputedStyle(page.body).writingMode;
    if (mode.startsWith('vertical')) {
      const bounds = page.body.getBoundingClientRect();
      const rect = node.getBoundingClientRect();
      return rect.left >= bounds.left - 0.5 && rect.right <= bounds.right + 0.5;
    }
    const bounds = pageBounds(page);
    return layoutBottom(node, bounds.scale) <= bounds.bottom;
  };

  const placeTableAcrossPages = (source: HTMLElement, setup: WordSectionSetup): void => {
    const sourceTable = source.querySelector<HTMLTableElement>(':scope > table');
    const sourceBody = sourceTable?.tBodies[0];
    if (!(sourceTable instanceof HTMLTableElement) || !sourceBody) {
      const page = ensurePage(setup);
      page.body.appendChild(source);
      source.classList.add('oversize-block');
      return;
    }

    const rowGroups = groupTableRows(Array.from(sourceBody.rows));
    if (!rowGroups.length) {
      const page = ensurePage(setup);
      page.body.appendChild(source);
      if (!fitsPage(source, page)) source.classList.add('oversize-block');
      return;
    }

    let groupIndex = 0;
    while (groupIndex < rowGroups.length) {
      const page = ensurePage(setup);
      const { wrap, body } = cloneTableFrame(source, sourceTable);
      page.body.appendChild(wrap);
      layoutText(wrap);
      let groupsOnPage = 0;

      while (groupIndex < rowGroups.length) {
        const group = rowGroups[groupIndex];
        body.append(...group);
        layoutText(wrap);
        if (fitsPage(wrap, page)) {
          groupIndex += 1;
          groupsOnPage += 1;
          continue;
        }

        if (group.length === 1 && group[0].dataset.cantSplit !== 'true') {
          const pair = splitTableRow(group[0], candidate => fitsPage(candidate, page), groupsOnPage > 0 || page.body.childElementCount > 1);
          if (pair) {
            group[0].replaceWith(pair.head);
            rowGroups[groupIndex] = [pair.tail];
            advancePage(setup);
            break;
          }
        }

        group.forEach(row => row.remove());
        if (groupsOnPage > 0) {
          advancePage(setup);
          break;
        }

        // 当前页已有内容时，先把完整行组移到新页，避免破坏跨行单元格。
        if (page.body.childElementCount > 1) {
          wrap.remove();
          advancePage(setup);
          break;
        }

        // 单个不可拆行组本身超过正文区时保留完整内容，并局部滚动兜底。
        body.append(...group);
        wrap.classList.add('oversize-block');
        groupIndex += 1;
        groupsOnPage += 1;
        if (groupIndex < rowGroups.length) advancePage(setup);
        break;
      }

      if (body.rows.length === 0) wrap.remove();
    }
  };

  const placeNodes = (nodes: HTMLElement[], setup: WordSectionSetup) => {
    for (let index = 0; index < nodes.length; index += 1) {
      let node = nodes[index];
      prepareImages(node);
      let page = ensurePage(setup);
      if (node.dataset.pageBreakBefore === 'true' && page.columns.some(column => column.childElementCount)) {
        page.column = page.columns.length - 1;
        page = advancePage(setup);
      }

      // Move a keep-with-next chain together when the complete chain fits a page.
      let end = index;
      while (end + 1 < nodes.length && nodes[end].dataset.keepNext === 'true'
        && nodes[end + 1].dataset.pageBreakBefore !== 'true') end += 1;
      if (end > index && page.body.childElementCount) {
        const group = nodes.slice(index, end + 1);
        page.body.append(...group);
        group.forEach(layoutText);
        const bounds = pageBounds(page);
        const bottom = layoutBottom(group[group.length - 1], bounds.scale);
        const top = group[0].getBoundingClientRect().top;
        const height = page.body.getBoundingClientRect().height;
        const following = group[group.length - 1];
        const minimumBottom = firstFlowLinesBottom(following, following.dataset.widowControl === 'false' ? 1 : 2);
        group.forEach(item => item.remove());
        if (bottom > bounds.bottom && (bottom - top <= height || minimumBottom > bounds.bottom)) page = advancePage(setup);
      }

      while (true) {
        const before = page.body.childElementCount;
        page.body.appendChild(node);
        layoutRunningImages(node, page.page, page.body);
        layoutText(node);
        if (fitsPage(node, page)) break;
        if (node.classList.contains('tbl-wrap')) {
          node.remove();
          placeTableAcrossPages(node, setup);
          break;
        }
        const split = splitFlowNode(node, candidate => fitsPage(candidate, page), before > 0);
        if (split) {
          node.replaceWith(split.head);
          node = split.tail;
          page = advancePage(setup);
          continue;
        }
        if (before > 0) {
          node.remove();
          page = advancePage(setup);
          continue;
        }
        // Images and indivisible table row groups still need a bounded fallback.
        node.classList.add('oversize-block');
        break;
      }
    }
  };

  const startSection = (setup: WordSectionSetup) => {
    if (!current) { current = newPage(setup, 1); return; }
    const previous = sections[sectionIndex - 1].setup;
    const kind = previous.breakType ?? 'nextPage';
    const sameGeometry = (['widthPt', 'heightPt', 'leftPt', 'rightPt', 'topPt', 'bottomPt'] as const)
      .every(key => previous.page[key] === setup.page[key])
      && (['columns', 'columnGapPt', 'columnSeparator', 'columnsRtl', 'textDirection', 'linePitchPt'] as const)
        .every(key => previous[key] === setup[key])
      && (['columnWidthsPt', 'columnSpacesPt'] as const).every(key => {
        const before = previous[key] ?? [], after = setup[key] ?? [];
        return before.length === after.length && before.every((value, index) => value === after[index]);
      });
    if (kind === 'continuous' && sameGeometry) {
      current.setup = setup;
      current.sectionPage = 1;
      return;
    }
    if (kind === 'nextColumn' && sameGeometry && current.column + 1 < current.columns.length) {
      advancePage(setup);
      current.setup = setup;
      current.sectionPage = 1;
      return;
    }
    const parity = kind === 'evenPage' ? 0 : kind === 'oddPage' ? 1 : undefined;
    if (parity !== undefined && (globalPage + 1) % 2 !== parity) newPage(previous, current.sectionPage + 1);
    current = newPage(setup, 1);
  };

  const pump = (initial = false) => {
    cancelPending = undefined;
    if (disposed || host.clientWidth === 0) return;
    // CSS zoom changes font rounding as well as rectangles. Lay out each batch
    // at a total zoom of 1, then restore the user's preview scale before paint.
    const probe = document.createElement('div');
    probe.style.cssText = 'position:absolute;width:960px;height:0;visibility:hidden';
    container.appendChild(probe);
    const scale = probe.getBoundingClientRect().width / 960;
    probe.remove();
    const previousZoom = container.style.zoom;
    if (scale > 0 && Math.abs(scale - 1) > 0.00001) container.style.zoom = String(1 / scale);
    try {
      const budget = initial ? FIRST_CHUNK : IDLE_CHUNK;
      let processed = 0;
      const started = performance.now();
      while (sectionIndex < sections.length && processed < budget && performance.now() - started < IDLE_BUDGET_MS) {
        const section = sections[sectionIndex];
        if (!sectionStarted) { startSection(section.setup); sectionStarted = true; }
        const nodes: HTMLElement[] = [];
        while (unitIndex < section.units.length && (processed < budget || nodes[nodes.length - 1]?.dataset.keepNext === 'true')) {
          const unit = section.units[unitIndex++];
          processed += 1;
          if (unit.kind === 'single' && (unit.block.kind === 'pageBreak' || unit.block.kind === 'columnBreak')) {
            placeNodes(nodes.splice(0), section.setup);
            if (unit.block.kind === 'pageBreak' && current) current.column = current.columns.length - 1;
            advancePage(section.setup);
            continue;
          }
          nodes.push(unit.kind === 'listRun' ? renderListRun(unit.items) : renderBlock(unit.block));
        }
        placeNodes(nodes, section.setup);
        if (unitIndex >= section.units.length) {
          sectionIndex += 1;
          unitIndex = 0;
          sectionStarted = false;
        }
      }
      if (sectionIndex < sections.length) cancelPending = scheduleIdle(() => pump(false));
      const pages = Array.from(container.querySelectorAll<HTMLElement>('.page'));
      for (const page of pages) {
        const bodies = page.querySelectorAll<HTMLElement>('.page-column');
        for (const body of bodies.length ? bodies : page.querySelectorAll<HTMLElement>('.page-body')) {
          layoutRunningImages(body, page, body);
        }
        const sectionPages = pages.filter(item => item.dataset.section === page.dataset.section).length;
        updatePageFields(page, pages.length, sectionPages);
      }
    } finally {
      container.style.zoom = previousZoom;
    }
  };
  const reflow = () => {
    if (disposed || reflowFrame !== undefined) return;
    reflowFrame = requestAnimationFrame(() => {
      reflowFrame = undefined;
      if (disposed) return;
      cancelPending?.();
      cancelPending = undefined;
      container.replaceChildren();
      sectionIndex = unitIndex = globalPage = 0;
      sectionStarted = false;
      current = undefined;
      pump(true);
    });
  };
  const imageLoaded = (event: Event) => {
    const image = event.target;
    if (!(image instanceof HTMLImageElement) || !image.naturalWidth || !image.naturalHeight
      || (image.hasAttribute('width') && image.hasAttribute('height')) || imageSizes.has(image.src)) return;
    imageSizes.set(image.src, { width: image.naturalWidth, height: image.naturalHeight });
    reflow();
  };
  const fonts = host.ownerDocument.fonts;
  fonts?.addEventListener('loadingdone', reflow);
  container.addEventListener('load', imageLoaded, true);
  if (fonts?.status === 'loading') void fonts.ready.then(() => { if (!disposed) pump(true); });
  else pump(true);

  return {
    stats: {
      blocks: (doc.blocks ?? []).length,
      paragraphs: doc.stats?.paragraphs ?? 0,
      tables: doc.stats?.tables ?? 0,
      images: doc.stats?.images ?? 0,
    },
    // Fixed page geometry needs no reflow; resume work deferred by a hidden host.
    resize() {
      if (!disposed && !cancelPending && sectionIndex < sections.length) pump(true);
    },
    getContentWidth() {
      if (disposed) return 0;
      const style = getComputedStyle(container);
      return pageWidth + (Number.parseFloat(style.paddingLeft) || 0) + (Number.parseFloat(style.paddingRight) || 0);
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      cancelPending?.();
      cancelPending = undefined;
      if (reflowFrame !== undefined) cancelAnimationFrame(reflowFrame);
      fonts?.removeEventListener('loadingdone', reflow);
      container.removeEventListener('load', imageLoaded, true);
      container.remove();
      style.remove();
    },
  };
}

function splitSections(doc: WordDoc): SectionRun[] {
  const sections: SectionRun[] = [];
  let blocks: WordBlock[] = [];
  for (const block of doc.blocks ?? []) {
    if (block.kind === 'sectionBreak' && block.section) {
      sections.push({ setup: block.section, units: groupUnits(blocks) });
      blocks = [];
    } else {
      blocks.push(block);
    }
  }
  sections.push({ setup: doc.setup, units: groupUnits(blocks) });
  return sections;
}

function groupUnits(blocks: WordBlock[]): RenderUnit[] {
  const units: RenderUnit[] = [];
  for (const block of blocks.flatMap(splitColumnBreaks)) {
    if (block.kind === 'listItem') {
      const last = units[units.length - 1];
      if (last?.kind === 'listRun' && !block.pageBreakBefore) last.items.push(block);
      else units.push({ kind: 'listRun', items: [block] });
    } else {
      units.push({ kind: 'single', block });
    }
  }
  return units;
}

function splitColumnBreaks(block: WordBlock): WordBlock[] {
  if (!block.columnBreakOffsets?.length || !block.content) return [block];
  const offsets = new Set(block.columnBreakOffsets);
  const result: WordBlock[] = [];
  let content: WordInline[] = [];
  let index = 0;
  let first = true;
  const flush = () => {
    if (content.length) result.push({ ...block, content, columnBreakOffsets: undefined,
      kind: !first && block.kind === 'listItem' ? 'paragraph' : block.kind,
      spacingBeforePt: first ? block.spacingBeforePt : 0, pageBreakBefore: first && block.pageBreakBefore });
    content = [];
    first = false;
  };
  for (const run of block.content) {
    if (!run.text) { content.push(run); continue; }
    let text = '';
    for (const char of run.text) {
      if (offsets.has(index)) {
        if (text) content.push({ ...run, text });
        text = '';
        flush();
        result.push({ kind: 'columnBreak' });
      } else text += char;
      index += 1;
    }
    if (text) content.push({ ...run, text });
  }
  flush();
  return result;
}

function runningBlocks(setup: WordSectionSetup, page: number, header: boolean): WordBlock[] {
  if (header) {
    if (page === 1 && setup.titlePage && setup.firstHeader.length) return setup.firstHeader;
    if (page % 2 === 0 && setup.evenHeader.length) return setup.evenHeader;
    return setup.header;
  }
  if (page === 1 && setup.titlePage && setup.firstFooter.length) return setup.firstFooter;
  if (page % 2 === 0 && setup.evenFooter.length) return setup.evenFooter;
  return setup.footer;
}

function appendRunningBlocks(parent: HTMLElement, blocks: WordBlock[]): void {
  for (const unit of groupUnits(blocks)) {
    parent.appendChild(unit.kind === 'listRun' ? renderListRun(unit.items) : renderBlock(unit.block));
  }
}

/** Floating header/footer drawings do not consume a place in the text line. */
function layoutRunningImages(root: HTMLElement, page: HTMLElement, body: HTMLElement): void {
  const images = Array.from(root.querySelectorAll<HTMLImageElement>('img[data-word-anchor]'));
  for (const image of images) {
    image.style.position = 'absolute';
    image.parentElement!.style.position = 'relative';
  }
  if (!images.length) return;
  const pageRect = page.getBoundingClientRect();
  const bodyRect = body.getBoundingClientRect();
  const scale = pageRect.width / parseFloat(getComputedStyle(page).width) || 1;
  for (const image of images) {
    const anchor: NonNullable<WordInline['anchor']> = JSON.parse(image.dataset.wordAnchor!);
    const paragraph = image.parentElement!.getBoundingClientRect();
    const left = anchor.horizontalFrom === 'page' ? pageRect.left : bodyRect.left;
    const top = anchor.verticalFrom === 'page' ? pageRect.top
      : anchor.verticalFrom === 'margin' ? bodyRect.top : paragraph.top;
    image.style.left = `${(left - paragraph.left) / scale + anchor.xPt * 4 / 3}px`;
    image.style.top = `${(top - paragraph.top) / scale + anchor.yPt * 4 / 3}px`;
  }
}

function scheduleIdle(cb: () => void): () => void {
  if (typeof globalThis.requestIdleCallback === 'function') {
    const id = globalThis.requestIdleCallback(cb);
    return () => globalThis.cancelIdleCallback?.(id);
  }
  const id = setTimeout(cb, 40);
  return () => clearTimeout(id);
}

// ---------- 块 ----------

function renderBlock(block: WordBlock): HTMLElement {
  switch (block.kind) {
    case 'heading': {
      const level = Math.min(Math.max(block.level ?? 2, 1), 6);
      const el = document.createElement(`h${level}`);
      applyParaStyle(el, block);
      renderInlines(block.content, el);
      return el;
    }
    case 'paragraph':
    case 'listItem': {
      const el = document.createElement('p');
      applyParaStyle(el, block);
      renderInlines(block.content, el);
      return el;
    }
    case 'image': {
      const wrap = document.createElement('div');
      wrap.className = 'img-wrap';
      const img = document.createElement('img');
      img.src = block.src ?? '';
      img.alt = block.alt ?? '图片';
      if (block.widthPx) img.width = block.widthPx;
      if (block.heightPx) img.height = block.heightPx;
      if (block.rotationDegrees) img.style.transform = `rotate(${block.rotationDegrees}deg)`;
      wrap.appendChild(img);
      return wrap;
    }
    case 'pageBreak':
    case 'columnBreak':
    case 'sectionBreak':
      return document.createElement('div');
    case 'table':
      return block.table ? renderTable(block.table) : document.createElement('div');
    case 'chart':
      return block.chart ? renderWordChart(block.chart) : document.createElement('div');
  }
}

/** 一段连续列表项 → 嵌套 ul/ol(端口自 rwml HTML 导出器的栈算法) */
function renderListRun(items: WordBlock[]): HTMLElement {
  const root = document.createElement('div');
  root.className = 'list-run';
  if (items[0]?.pageBreakBefore) root.dataset.pageBreakBefore = 'true';
  const stack: { ordered: boolean; el: HTMLElement }[] = [];
  const numbers: number[] = [];
  for (const item of items) {
    const info = item.list;
    if (!info) continue;
    while (stack.length > info.level + 1) stack.pop();
    if (stack.length < info.level + 1) {
      while (stack.length < info.level + 1) {
        const el = info.ordered ? document.createElement('ol') : document.createElement('ul');
        (stack[stack.length - 1]?.el ?? root).appendChild(el);
        stack.push({ ordered: info.ordered, el });
      }
    } else if (stack[stack.length - 1].ordered !== info.ordered) {
      stack.pop();
      const el = info.ordered ? document.createElement('ol') : document.createElement('ul');
      (stack[stack.length - 1]?.el ?? root).appendChild(el);
      stack.push({ ordered: info.ordered, el });
    }
    const li = document.createElement('li');
    if (info.label) {
      li.dataset.marker = info.label;
      if (info.markerFont) {
        li.style.setProperty('--marker-font-family', JSON.stringify(info.markerFont));
      }
    } else if (info.ordered) {
      numbers.length = info.level + 1;
      numbers[info.level] = (numbers[info.level] ?? 0) + 1;
      li.dataset.marker = `${Array.from({ length: info.level + 1 }, (_, level) => numbers[level] ?? 1).join('.')}.`;
    }
    // 缩进优先用文档值(左缩进 + 悬挂),缺失时按级别兜底 21pt/级
    const left = item.indentLeftPt ?? (info.level + 1) * 21;
    const first = item.indentFirstPt != null ? item.indentFirstPt
      : item.hangingPt != null ? -item.hangingPt : null;
    li.style.marginLeft = `${left}pt`;
    if (first != null && first !== 0) li.style.textIndent = `${first}pt`;
    applyParaStyle(li, item);
    renderInlines(item.content, li);
    stack[stack.length - 1].el.appendChild(li);
  }
  if (items[items.length - 1]?.pagination?.keepNext) root.dataset.keepNext = 'true';
  return root;
}

// ---------- 段落级版式 ----------

function applyParaStyle(el: HTMLElement, b: WordBlock): void {
  const s: string[] = [];
  if (b.pageBreakBefore) el.dataset.pageBreakBefore = 'true';
  if (b.pagination) {
    el.dataset.keepNext = String(b.pagination.keepNext);
    el.dataset.keepLines = String(b.pagination.keepLines);
    el.dataset.widowControl = String(b.pagination.widowControl);
  }
  if (b.align) s.push(`text-align:${b.align}`);
  if (b.shading) s.push(`background-color:${b.shading}`);
  if (b.bidi) { el.dir = 'rtl'; s.push('unicode-bidi:plaintext'); }
  if (b.indentLeftPt != null) s.push(`margin-left:${b.indentLeftPt}pt`);
  if (b.indentRightPt != null) s.push(`margin-right:${b.indentRightPt}pt`);
  if (b.content?.some(run => run.text?.includes('\t'))) {
    el.dataset.wordTabs = JSON.stringify((b.tabStops ?? []).slice().sort((a, b) => a.positionPt - b.positionPt));
  }
  // Word 语义:首行缩进可正(普通)可负(悬挂,编号在左文字在右)
  const firstLine = b.indentFirstPt != null ? b.indentFirstPt : b.hangingPt != null ? -b.hangingPt : null;
  if (firstLine != null && firstLine !== 0) s.push(`text-indent:${firstLine}pt`);
  if (b.linePct != null) {
    s.push(`line-height:${b.linePct}`);
    if (!b.lineSpacing) el.dataset.wordLineMultiple = String(b.linePct);
  }
  if (b.lineSpacing) {
    el.dataset.lineSpacing = b.lineSpacing.rule;
    s.push(`line-height:${b.lineSpacing.valuePt}pt`);
  }
  if (b.spacingBeforePt != null) s.push(`margin-top:${b.spacingBeforePt}pt`);
  if (b.spacingAfterPt != null) s.push(`margin-bottom:${b.spacingAfterPt}pt`);
  if (s.length) el.setAttribute('style', (el.getAttribute('style') ?? '') + ';' + s.join(';'));
}

// ---------- 行内 ----------

function renderInlines(inlines: WordInline[] | undefined, parent: HTMLElement): void {
  if (!inlines) return;
  for (const inline of inlines) parent.appendChild(renderInline(inline));
}

function renderInline(inline: WordInline): Node {
  if (inline.kind === 'image') {
    const img = document.createElement('img');
    img.src = inline.src ?? '';
    img.alt = inline.alt ?? '图片';
    if (inline.widthPx) img.width = inline.widthPx;
    if (inline.heightPx) img.height = inline.heightPx;
    if (inline.rotationDegrees) img.style.transform = `rotate(${inline.rotationDegrees}deg)`;
    if (inline.anchor) {
      img.dataset.wordAnchor = JSON.stringify(inline.anchor);
      img.style.position = 'absolute';
    }
    return img;
  }
  let el: HTMLElement = document.createElement('span');
  el.dataset.wordRun = 'true';
  applyRunStyle(el, inline);
  if (inline.field) {
    el.dataset.wordField = inline.field.kind;
    if (inline.field.format) el.dataset.fieldFormat = inline.field.format;
  }
  appendTabbedText(el, inline.text ?? '');
  if (inline.vertAlign === 'super' || inline.vertAlign === 'sub') {
    const tag = document.createElement(inline.vertAlign === 'super' ? 'sup' : 'sub');
    tag.appendChild(el);
    el = tag;
  }
  if (inline.kind === 'link') {
    const a = document.createElement('a');
    a.href = inline.href ?? '#';
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
    a.appendChild(el);
    return a;
  }
  return el;
}

/** 富字符样式:字体/字号/颜色/高亮/粗斜下删 */
function applyRunStyle(el: HTMLElement, r: WordInline): void {
  const s: string[] = [];
  if (r.font) { s.push(`font-family:${cssFont(r.font)}`); el.dataset.wordFont = r.font; }
  if (r.sizeHalfPt != null) s.push(`font-size:${r.sizeHalfPt / 2}pt`);
  if (r.color) s.push(`color:${r.color}`);
  if (r.highlight) s.push(`background:${HIGHLIGHT[r.highlight] ?? '#ffeb3b'}`);
  if (r.caps) s.push('text-transform:uppercase');
  if (r.smallCaps) s.push('font-variant-caps:small-caps');
  if (r.rtl) { el.dir = 'rtl'; s.push('unicode-bidi:isolate'); }
  if (r.bold) el.classList.add('b');
  if (r.italic) el.classList.add('i');
  if (r.underline) el.classList.add('u');
  if (r.strike) el.classList.add('s');
  if (s.length) el.setAttribute('style', s.join(';'));
}

/** 中文/带空格字体名加引号,并补 CJK 回退 */
function cssFont(font: string): string {
  const needsQuote = /[^A-Za-z0-9-]/.test(font);
  const name = needsQuote ? `"${font.replace(/"/g, '')}"` : font;
  // Word's SimSun names are absent on macOS. Keep a Song typeface before the
  // sans-serif CJK fallback, whose wider spaces can wrap aligned headers.
  if (/^(宋体|新宋体|simsun|nsimsun)$/i.test(font)) {
    return `${name}, SimSun, "Songti SC", STSong, "Noto Serif CJK SC", serif`;
  }
  return `${name}, "PingFang SC", "Microsoft YaHei", sans-serif`;
}

// ---------- 表格 ----------

function renderTable(t: WordTable): HTMLElement {
  const wrap = document.createElement('div');
  wrap.className = 'tbl-wrap';
  // Cloned page fragments keep their logical table/cell identity for selection
  // and verification without mistaking repeated headers for extra source rows.
  wrap.dataset.wordTableId = String(++tableSequence);
  const el = document.createElement('table');
  if (t.bidiVisual) el.dir = 'rtl';
  // 表宽:文档比例(缺省 100%);固定布局用 table-layout:fixed
  el.style.width = `${((t.widthPct ?? 1) * 100).toFixed(2)}%`;
  if (t.fixedLayout) el.style.tableLayout = 'fixed';
  // 表格缩进(twips→pt):左对齐时的左缩进;对齐优先
  if (t.indentPt != null && t.indentPt !== 0) el.style.marginLeft = `${t.indentPt}pt`;
  if (t.align === 'center') { el.style.marginLeft = 'auto'; el.style.marginRight = 'auto'; }
  else if (t.align === 'right') { el.style.marginLeft = 'auto'; }
  // 列宽比例(文档列宽优先,单元格宽兜底)
  if (t.colWidthsPct?.length) {
    const colgroup = document.createElement('colgroup');
    for (const pct of t.colWidthsPct) {
      const col = document.createElement('col');
      col.style.width = `${(pct * 100).toFixed(2)}%`;
      colgroup.appendChild(col);
    }
    el.appendChild(colgroup);
  }

  const headerRows = Math.min(t.headerRows ?? 0, t.grid.length);
  if (headerRows > 0) {
    const thead = document.createElement('thead');
    buildRows(thead, t, 0, headerRows);
    el.appendChild(thead);
  }
  const tbody = document.createElement('tbody');
  buildRows(tbody, t, headerRows, t.grid.length);
  el.appendChild(tbody);
  wrap.appendChild(el);
  return wrap;
}

function buildRows(parent: HTMLTableSectionElement, t: WordTable, from: number, to: number): void {
  for (let r = from; r < to; r++) {
    const tr = document.createElement('tr');
    tr.dataset.wordRow = String(r);
    if (t.rowCantSplit?.[r]) tr.dataset.cantSplit = 'true';
    const minimumHeight = t.rowMinHeightsPt?.[r];
    // CSS table row height is a minimum: content can still grow naturally.
    if (minimumHeight != null && Number.isFinite(minimumHeight) && minimumHeight > 0) {
      tr.style.height = `${minimumHeight}pt`;
    }
    for (const [column, slot] of t.grid[r].entries()) {
      if (slot.kind !== 'origin' || !slot.cell) continue;
      const cell = buildCell(slot.cell);
      cell.dataset.wordColumn = String(column);
      tr.appendChild(cell);
    }
    parent.appendChild(tr);
  }
}

/** 将跨行单元格覆盖的连续行视为不可拆分行组。 */
function groupTableRows(rows: HTMLTableRowElement[]): HTMLTableRowElement[][] {
  const groups: HTMLTableRowElement[][] = [];
  let start = 0;
  let coveredUntil = 0;
  for (let index = 0; index < rows.length; index++) {
    const maxRowSpan = Math.max(1, ...Array.from(rows[index].cells, cell => cell.rowSpan || 1));
    coveredUntil = Math.max(coveredUntil, index + maxRowSpan);
    if (index + 1 >= coveredUntil) {
      groups.push(rows.slice(start, index + 1));
      start = index + 1;
    }
  }
  if (start < rows.length) groups.push(rows.slice(start));
  return groups;
}

function cloneTableFrame(
  sourceWrap: HTMLElement,
  sourceTable: HTMLTableElement,
): { wrap: HTMLElement; body: HTMLTableSectionElement } {
  const wrap = sourceWrap.cloneNode(false) as HTMLElement;
  const table = sourceTable.cloneNode(false) as HTMLTableElement;
  for (const child of Array.from(sourceTable.children)) {
    if (child.tagName === 'COLGROUP' || child.tagName === 'THEAD') {
      table.appendChild(child.cloneNode(true));
    }
  }
  const body = document.createElement('tbody');
  table.appendChild(body);
  wrap.appendChild(table);
  return { wrap, body };
}

/** Split an unmerged, splittable row at its cell block/line boundaries. */
function splitTableRow(
  source: HTMLTableRowElement,
  fits: (node: HTMLElement) => boolean,
  hasPrevious: boolean,
): { head: HTMLTableRowElement; tail: HTMLTableRowElement } | undefined {
  if (Array.from(source.cells).some(cell => cell.rowSpan > 1)) return;
  const minimumHeightPt = source.style.height.endsWith('pt') ? parseFloat(source.style.height) : 0;
  const head = source.cloneNode(true) as HTMLTableRowElement;
  const tail = source.cloneNode(false) as HTMLTableRowElement;
  // Measure the head by content, then carry only the remaining source minimum
  // onto the continuation so splitting does not discard or duplicate space.
  head.style.removeProperty('height');
  tail.style.removeProperty('height');
  let hasHead = false;
  let hasTail = false;
  source.replaceWith(head);
  try {
    for (const cell of Array.from(head.cells)) {
      const continuation = cell.cloneNode(false) as HTMLTableCellElement;
      tail.appendChild(continuation);
      cell.style.verticalAlign = 'top';
      continuation.style.verticalAlign = 'top';
      let overflow = false;
      for (const block of Array.from(cell.children) as HTMLElement[]) {
        if (overflow) { continuation.appendChild(block); continue; }
        if (fits(block)) { hasHead = true; continue; }
        const pair = splitFlowNode(block, fits, hasPrevious);
        if (pair) {
          block.replaceWith(pair.head);
          continuation.appendChild(pair.tail);
          hasHead = true;
        } else continuation.appendChild(block);
        overflow = true;
      }
      if (continuation.textContent || continuation.querySelector('img,table,svg')) hasTail = true;
    }
    if (hasHead && hasTail && fits(head)) {
      const headHeightPt = parseFloat(getComputedStyle(head).height) * 3 / 4;
      const remaining = minimumHeightPt - headHeightPt;
      if (remaining > 0) tail.style.height = `${remaining}pt`;
      return { head, tail };
    }
  } finally { head.replaceWith(source); }
  return undefined;
}

function buildCell(cell: WordCell): HTMLTableCellElement {
  const td = cell.isHeader ? document.createElement('th') : document.createElement('td');
  if (cell.colSpan > 1) td.colSpan = cell.colSpan;
  if (cell.rowSpan > 1) td.rowSpan = cell.rowSpan;
  const s: string[] = [];
  if (cell.shading) s.push(`background-color:${cell.shading}`);

  if (s.length) td.setAttribute('style', s.join(';'));
  for (const side of ['top', 'right', 'bottom', 'left'] as const) {
    const border = cell.borders?.[side];
    if (border) td.style.setProperty(`border-${side}`, `${border.widthPt ?? 0.5}pt ${border.style} ${border.color ?? '#000000'}`);
    if (!border || border.style === 'none' || border.widthPt === 0) {
      td.classList.add('word-cell-grid');
      td.style.setProperty(`--word-grid-${side}`, `var(--word-grid-${side === 'top' || side === 'bottom' ? 'horizontal' : 'vertical'})`);
    }
  }
  if (cell.valign) td.style.verticalAlign = cell.valign;
  if (cell.marginTopPt != null) td.style.paddingTop = `${cell.marginTopPt}pt`;
  if (cell.marginRightPt != null) td.style.paddingRight = `${cell.marginRightPt}pt`;
  if (cell.marginBottomPt != null) td.style.paddingBottom = `${cell.marginBottomPt}pt`;
  if (cell.marginLeftPt != null) td.style.paddingLeft = `${cell.marginLeftPt}pt`;
  appendRunningBlocks(td, cell.blocks);
  return td;
}

/** Resolve cell indents after attachment, before measuring tabs or pagination. */
function layoutTableCellIndents(root: HTMLElement): void {
  const selector = 'p,li,h1,h2,h3,h4,h5,h6';
  const paragraphs = root.matches(selector) ? [root] : root.querySelectorAll<HTMLElement>(selector);
  for (const paragraph of paragraphs) {
    const indent = Number.parseFloat(paragraph.style.textIndent);
    if (!(indent > 0) || !paragraph.style.textIndent.endsWith('pt') || !paragraph.closest('td,th')) continue;
    const width = Number.parseFloat(getComputedStyle(paragraph).width);
    // An indent beyond the whole cell text area cannot be honored. Fall back
    // to its origin instead of leaving a one-character first line at the edge.
    // CSS content width accounts for paragraph margins and is independent of zoom.
    if (width > 0 && indent * 4 / 3 >= width) {
      paragraph.style.textIndent = '0pt';
    }
  }
}

function sectionWritingMode(direction?: string): string | undefined {
  if (direction === 'topToBottomRightToLeft' || direction === 'topToBottomRightToLeftVertical') return 'vertical-rl';
  if (direction === 'bottomToTopLeftToRight' || direction === 'topToBottomLeftToRightVertical') return 'vertical-lr';
  return undefined;
}

function updatePageFields(page: HTMLElement, total: number, sectionTotal: number): void {
  for (const field of page.querySelectorAll<HTMLElement>('[data-word-field]')) {
    const value = field.dataset.wordField === 'NUMPAGES' ? total : field.dataset.wordField === 'SECTIONPAGES'
      ? sectionTotal : Number(page.dataset.pageNumber ?? 1);
    field.textContent = formatPageNumber(value, field.dataset.fieldFormat ?? page.dataset.pageFormat);
  }
}

function formatPageNumber(value: number, format = 'decimal'): string {
  if (format === 'lowerRoman' || format === 'upperRoman' || format === 'roman' || format === 'ROMAN') {
    let rest = value;
    let result = '';
    for (const [number, digits] of [[1000, 'M'], [900, 'CM'], [500, 'D'], [400, 'CD'], [100, 'C'], [90, 'XC'], [50, 'L'], [40, 'XL'], [10, 'X'], [9, 'IX'], [5, 'V'], [4, 'IV'], [1, 'I']] as const) {
      while (rest >= number && result.length < 10000) { result += digits; rest -= number; }
    }
    return format === 'lowerRoman' || format === 'roman' ? result.toLowerCase() : result;
  }
  if (['upperLetter', 'lowerLetter', 'ALPHABETIC', 'alphabetic'].includes(format)) {
    let result = '';
    for (let number = value; number > 0; number = Math.floor((number - 1) / 26)) result = String.fromCharCode(65 + (number - 1) % 26) + result;
    return format === 'lowerLetter' || format === 'alphabetic' ? result.toLowerCase() : result;
  }
  if (format === 'decimalZero') return String(value).padStart(2, '0');
  if (format === 'numberInDash') return `- ${value} -`;
  if (format === 'decimalFullWidth' || format === 'decimalFullWidth2') return String(value).replace(/\d/g, digit => String.fromCharCode(digit.charCodeAt(0) + 0xfee0));
  return String(value);
}
