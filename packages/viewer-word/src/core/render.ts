import { SHADOW_CSS } from './styles';
import type { WordBlock, WordCell, WordDoc, WordInline, WordSectionSetup, WordTable } from './types';

const FIRST_CHUNK = 120;
const IDLE_CHUNK = 80;
const IDLE_BUDGET_MS = 50;

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
type PageState = { body: HTMLElement; setup: WordSectionSetup; sectionPage: number };

/**
 * 渲染入口:Shadow DOM 隔离 + 渲染单元分批挂载。
 * 字体/字号/颜色/高亮/对齐/缩进/行距全部映射为内联样式。
 */
export function renderWordInto(host: HTMLElement, doc: WordDoc): {
  stats: { blocks: number; paragraphs: number; tables: number; images: number };
  resize(): void;
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

  const sections = splitSections(doc);
  let sectionIndex = 0;
  let unitIndex = 0;
  let globalPage = 0;
  let current: PageState | undefined;
  let disposed = false;
  let cancelPending: (() => void) | undefined;

  const newPage = (setup: WordSectionSetup, sectionPage: number): PageState => {
    globalPage += 1;
    const page = document.createElement('section');
    page.className = 'page';
    page.style.width = `${setup.page.widthPt}pt`;
    page.style.height = `${setup.page.heightPt}pt`;

    const header = document.createElement('header');
    header.className = 'page-header';
    header.style.left = `${setup.page.leftPt}pt`;
    header.style.right = `${setup.page.rightPt}pt`;
    header.style.top = `${Math.max(6, setup.page.topPt * 0.2)}pt`;
    appendRunningBlocks(header, runningBlocks(setup, sectionPage, true));

    const body = document.createElement('main');
    body.className = 'page-body';
    body.style.left = `${setup.page.leftPt}pt`;
    body.style.right = `${setup.page.rightPt}pt`;
    body.style.top = `${setup.page.topPt}pt`;
    body.style.bottom = `${setup.page.bottomPt}pt`;
    if (setup.linePitchPt) body.style.lineHeight = `${setup.linePitchPt}pt`;

    const footer = document.createElement('footer');
    footer.className = 'page-footer';
    footer.style.left = `${setup.page.leftPt}pt`;
    footer.style.right = `${setup.page.rightPt}pt`;
    footer.style.bottom = `${Math.max(6, setup.page.bottomPt * 0.2)}pt`;
    const footerBlocks = runningBlocks(setup, sectionPage, false);
    appendRunningBlocks(footer, footerBlocks);
    if (setup.pageNumbers && footerBlocks.length === 0) {
      const number = document.createElement('span');
      number.className = 'page-number';
      number.textContent = String(globalPage);
      footer.appendChild(number);
    }

    page.append(header, body, footer);
    container.appendChild(page);
    return { body, setup, sectionPage };
  };

  const ensurePage = (setup: WordSectionSetup): PageState => {
    current ??= newPage(setup, 1);
    return current;
  };

  const advancePage = (setup: WordSectionSetup): PageState => {
    const next = (current?.sectionPage ?? 0) + 1;
    current = newPage(setup, next);
    return current;
  };

  const layoutBottom = (node: HTMLElement): number => {
    const marginBottom = Number.parseFloat(getComputedStyle(node).marginBottom) || 0;
    return node.getBoundingClientRect().bottom + marginBottom;
  };

  const fitsPage = (node: HTMLElement, page: PageState): boolean =>
    layoutBottom(node) <= page.body.getBoundingClientRect().bottom + 0.5;

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
      let groupsOnPage = 0;

      while (groupIndex < rowGroups.length) {
        const group = rowGroups[groupIndex];
        body.append(...group);
        if (fitsPage(wrap, page)) {
          groupIndex += 1;
          groupsOnPage += 1;
          continue;
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
    let remaining = nodes;
    while (remaining.length) {
      const page = ensurePage(setup);
      const before = page.body.childElementCount;
      page.body.append(...remaining);
      const bodyBottom = page.body.getBoundingClientRect().bottom + 0.5;
      let overflowAt = remaining.findIndex(node => layoutBottom(node) > bodyBottom);
      if (overflowAt < 0) return;
      const overflow = remaining.slice(overflowAt);
      overflow.forEach(node => node.remove());
      const firstOverflow = overflow[0];
      if (firstOverflow?.classList.contains('tbl-wrap')) {
        placeTableAcrossPages(firstOverflow, setup);
        remaining = overflow.slice(1);
        continue;
      }
      if (overflowAt === 0 && before === 0) {
        firstOverflow.classList.add('oversize-block');
        page.body.appendChild(firstOverflow);
        remaining = overflow.slice(1);
        if (remaining.length) advancePage(setup);
        continue;
      }
      advancePage(setup);
      remaining = overflow;
    }
  };

  const pump = (initial = false) => {
    cancelPending = undefined;
    if (disposed || host.clientWidth === 0) return;
    const budget = initial ? FIRST_CHUNK : IDLE_CHUNK;
    let processed = 0;
    const started = performance.now();
    while (sectionIndex < sections.length && processed < budget && performance.now() - started < IDLE_BUDGET_MS) {
      const section = sections[sectionIndex];
      if (!current) current = newPage(section.setup, 1);
      const nodes: HTMLElement[] = [];
      while (unitIndex < section.units.length && processed < budget) {
        const unit = section.units[unitIndex++];
        processed += 1;
        if (unit.kind === 'single' && unit.block.kind === 'pageBreak') {
          placeNodes(nodes.splice(0), section.setup);
          if (current.body.childElementCount > 0) advancePage(section.setup);
          continue;
        }
        nodes.push(unit.kind === 'listRun' ? renderListRun(unit.items) : renderBlock(unit.block));
      }
      placeNodes(nodes, section.setup);
      if (unitIndex >= section.units.length) {
        sectionIndex += 1;
        unitIndex = 0;
        current = undefined;
      }
    }
    if (sectionIndex < sections.length) cancelPending = scheduleIdle(() => pump(false));
  };
  pump(true);

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
    dispose() {
      if (disposed) return;
      disposed = true;
      cancelPending?.();
      cancelPending = undefined;
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
  for (const block of blocks) {
    if (block.kind === 'listItem') {
      const last = units[units.length - 1];
      if (last?.kind === 'listRun') last.items.push(block);
      else units.push({ kind: 'listRun', items: [block] });
    } else {
      units.push({ kind: 'single', block });
    }
  }
  return units;
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
      wrap.appendChild(img);
      return wrap;
    }
    case 'pageBreak':
    case 'sectionBreak':
      return document.createElement('div');
    case 'table':
      return block.table ? renderTable(block.table) : document.createElement('div');
  }
}

/** 一段连续列表项 → 嵌套 ul/ol(端口自 rwml HTML 导出器的栈算法) */
function renderListRun(items: WordBlock[]): HTMLElement {
  const root = document.createElement('div');
  root.className = 'list-run';
  const stack: { ordered: boolean; el: HTMLElement }[] = [];
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
  return root;
}

// ---------- 段落级版式 ----------

function applyParaStyle(el: HTMLElement, b: WordBlock): void {
  const s: string[] = [];
  if (b.align) s.push(`text-align:${b.align}`);
  if (b.indentLeftPt != null) s.push(`margin-left:${b.indentLeftPt}pt`);
  // Word 语义:首行缩进可正(普通)可负(悬挂,编号在左文字在右)
  const firstLine = b.indentFirstPt != null ? b.indentFirstPt : b.hangingPt != null ? -b.hangingPt : null;
  if (firstLine != null && firstLine !== 0) s.push(`text-indent:${firstLine}pt`);
  if (b.linePct != null) s.push(`line-height:${b.linePct}`);
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
    return img;
  }
  let el: HTMLElement = document.createElement('span');
  applyRunStyle(el, inline);
  el.textContent = inline.text ?? '';
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
  if (r.font) s.push(`font-family:${cssFont(r.font)}`);
  if (r.sizeHalfPt != null) s.push(`font-size:${r.sizeHalfPt / 2}pt`);
  if (r.color) s.push(`color:${r.color}`);
  if (r.highlight) s.push(`background:${HIGHLIGHT[r.highlight] ?? '#ffeb3b'}`);
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
  return `${name}, "PingFang SC", "Microsoft YaHei", sans-serif`;
}

// ---------- 表格 ----------

function renderTable(t: WordTable): HTMLElement {
  const wrap = document.createElement('div');
  wrap.className = 'tbl-wrap';
  const el = document.createElement('table');
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
    for (const slot of t.grid[r]) {
      if (slot.kind !== 'origin' || !slot.cell) continue;
      tr.appendChild(buildCell(slot.cell));
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

function buildCell(cell: WordCell): HTMLTableCellElement {
  const td = cell.isHeader ? document.createElement('th') : document.createElement('td');
  if (cell.colSpan > 1) td.colSpan = cell.colSpan;
  if (cell.rowSpan > 1) td.rowSpan = cell.rowSpan;
  const s: string[] = [];
  if (cell.shading) s.push(`background:${cell.shading}`);

  if (s.length) td.setAttribute('style', s.join(';'));
  if (cell.valign) td.style.verticalAlign = cell.valign;
  if (cell.marginTopPt != null) td.style.paddingTop = `${cell.marginTopPt}pt`;
  if (cell.marginRightPt != null) td.style.paddingRight = `${cell.marginRightPt}pt`;
  if (cell.marginBottomPt != null) td.style.paddingBottom = `${cell.marginBottomPt}pt`;
  if (cell.marginLeftPt != null) td.style.paddingLeft = `${cell.marginLeftPt}pt`;
  for (const block of cell.blocks) {
    // 单元格内容:块级递归(单元格内列表罕见,按段落渲染)
    td.appendChild(renderBlock(block));
  }
  return td;
}
