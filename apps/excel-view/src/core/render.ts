import { formatValue } from './format';
import { SHADOW_CSS } from './styles';
import type { ExcelCell, ExcelDoc, ExcelRichTextRun, ExcelShape, ExcelSheet } from './types';

/** 默认列宽换算:Excel 字符单位 → px(Calibri 11 近似) */
const DEFAULT_COL_PX = 96;
const DEFAULT_ROW_PX = 26;

const BORDER_MAP: Record<string, string> = {
  thin: '1px solid #9aa3ab',
  medium: '2px solid #666',
  thick: '3px solid #444',
  hair: '1px solid #d0d7de',
  dashed: '1px dashed #888',
  dotted: '1px dotted #888',
  double: '3px double #888',
};

const ALIGN_H: Record<string, string> = { Center: 'center', Right: 'end', Left: 'start' };

export interface RenderStats {
  sheets: number;
  rows: number;
  cols: number;
  merges: number;
  images: number;
  shapes: number;
}

/**
 * 网格渲染:完整网格(每格有边框)+ 合并 span + 列宽行高 + 全样式映射。
 * Shadow DOM 隔离;工作表逐个挂载(单元格量小,无需分批)。
 */
export function renderExcelInto(host: HTMLElement, doc: ExcelDoc): {
  stats: RenderStats;
  dispose(): void;
} {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  shadow.replaceChildren();

  const style = document.createElement('style');
  style.textContent = SHADOW_CSS;
  shadow.appendChild(style);

  const root = document.createElement('div');
  root.className = 'workbook';
  shadow.appendChild(root);

  const tabs = document.createElement('div');
  tabs.className = 'sheet-tabs';
  tabs.setAttribute('role', 'tablist');
  const sheets = document.createElement('div');
  sheets.className = 'sheet-pages';
  root.append(tabs, sheets);

  let rows = 0;
  let cols = 0;
  let merges = 0;
  let images = 0;
  let shapes = 0;
  const sections: HTMLElement[] = [];
  const buttons: HTMLButtonElement[] = [];
  doc.sheets.forEach((sheet, index) => {
    const section = renderSheet(sheet);
    section.hidden = index !== 0;
    sections.push(section);
    sheets.appendChild(section);
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'sheet-tab';
    button.textContent = sheet.name;
    button.setAttribute('role', 'tab');
    button.setAttribute('aria-selected', index === 0 ? 'true' : 'false');
    button.addEventListener('click', () => {
      sections.forEach((item, itemIndex) => { item.hidden = itemIndex !== index; });
      buttons.forEach((item, itemIndex) => item.setAttribute('aria-selected', itemIndex === index ? 'true' : 'false'));
    });
    buttons.push(button);
    tabs.appendChild(button);
    rows += sheet.highestRow;
    cols += sheet.highestCol;
    merges += sheet.merges.length;
    images += sheet.images.length;
    shapes += sheet.shapes.length;
  });


  return {
    stats: { sheets: doc.sheets.length, rows, cols, merges, images, shapes },
    dispose: () => shadow.replaceChildren(),
  };
}

function parseRange(range: string): { c1: number; r1: number; c2: number; r2: number } {
  const [a, b] = range.split(':');
  const m = (s: string) => {
    const t = s.match(/^([A-Z]+)(\d+)$/)!;
    let c = 0;
    for (const ch of t[1]) c = c * 26 + (ch.charCodeAt(0) - 64);
    return { c, r: Number(t[2]) };
  };
  const x = m(a);
  const y = b ? m(b) : x;
  return { c1: x.c, r1: x.r, c2: y.c, r2: y.r };
}

function renderSheet(sheet: ExcelSheet): HTMLElement {
  const section = document.createElement('section');
  section.className = 'sheet';
  const title = document.createElement('h2');
  title.textContent = `📋 ${sheet.name}(${sheet.highestRow} 行 × ${sheet.highestCol} 列${sheet.merges.length ? `,合并 ${sheet.merges.length} 区` : ''})`;
  section.appendChild(title);

  const wrap = document.createElement('div');
  wrap.className = 'grid-wrap';
  const grid = document.createElement('div');
  grid.className = 'grid';
  // 网格本体补左/顶边框,单元格画右/下,形成完整栅格
  const colPx = (c: number) => {
    const dim = sheet.colWidths.find(w => w.col === c);
    return dim && dim.width > 0 ? Math.round(dim.width * 7 + 5) : DEFAULT_COL_PX;
  };
  const rowPx = (r: number) => {
    const dim = sheet.rowHeights.find(h => h.row === r);
    return dim && dim.height > 0 ? Math.round(dim.height * (96 / 72)) : DEFAULT_ROW_PX;
  };
  // wrap 列保持文档宽(换行吸收宽度),其余列:文档宽为下限、内容宽为上限
  const wrapCols = new Set<number>();
  for (const cell of sheet.cells) {
    if (cell.wrapText) wrapCols.add(cell.col);
  }
  grid.style.gridTemplateColumns = Array.from({ length: sheet.highestCol }, (_, i) => {
    const c = i + 1;
    return wrapCols.has(c) ? `${colPx(c)}px` : `minmax(${colPx(c)}px, max-content)`;
  }).join(' ');

  // 合并区 → span + 覆盖集合
  const covered = new Set<string>();
  const spanOf = new Map<string, { cs: number; rs: number }>();
  for (const range of sheet.merges) {
    const { c1, r1, c2, r2 } = parseRange(range);
    spanOf.set(`${c1}:${r1}`, { cs: c2 - c1 + 1, rs: r2 - r1 + 1 });
    for (let r = r1; r <= r2; r++) {
      for (let c = c1; c <= c2; c++) {
        if (!(r === r1 && c === c1)) covered.add(`${c}:${r}`);
      }
    }
  }

  const byKey = new Map(sheet.cells.map(c => [`${c.col}:${c.row}`, c]));
  const wrapRows = new Set<number>();
  const explicitRowPx = new Set<number>();
  for (let r = 1; r <= sheet.highestRow; r++) {
    for (let c = 1; c <= sheet.highestCol; c++) {
      const key = `${c}:${r}`;
      if (covered.has(key)) continue;
      const cell = byKey.get(key);
      const el = document.createElement('div');
      el.className = 'cell';
      el.style.gridColumn = `${c}`;
      el.style.gridRow = `${r}`;
      const span = spanOf.get(key);
      if (span) {
        if (span.cs > 1) el.style.gridColumn = `${c} / span ${span.cs}`;
        if (span.rs > 1) el.style.gridRow = `${r} / span ${span.rs}`;
      }
      if (cell?.richText?.length) renderRichText(el, cell.richText);
      else el.textContent = cell ? formatValue(cell.value, cell.numfmt) : '';
      if (cell) {
        applyCellStyle(el, cell);
        if (cell.wrapText) {
          // Excel 自动换行:多行显示;行高仅在文档未显式设定时自适应
          el.style.whiteSpace = 'normal';
          wrapRows.add(r);
        }
      }
      if (r <= sheet.frozenRows) {
        el.classList.add('frozen-row');
        el.style.top = `${Array.from({ length: r - 1 }, (_, index) => rowPx(index + 1)).reduce((sum, value) => sum + value, 0)}px`;
      }
      if (c <= sheet.frozenColumns) {
        el.classList.add('frozen-column');
        el.style.left = `${Array.from({ length: c - 1 }, (_, index) => colPx(index + 1)).reduce((sum, value) => sum + value, 0)}px`;
      }
      if (r <= sheet.frozenRows && c <= sheet.frozenColumns) el.classList.add('frozen-corner');
      grid.appendChild(el);
    }
  }
  // 行高:文档行高(或默认)为下限、内容高度为上限 —— 显式优先且永不裁切
  for (const dim of sheet.rowHeights) {
    if (dim.height > 0) explicitRowPx.add(dim.row);
  }
  void explicitRowPx; void wrapRows;
  for (let r = 1; r <= sheet.highestRow; r++) {
    grid.style.gridTemplateRows += ` minmax(${rowPx(r)}px, auto)`;
  }

  for (const source of sheet.images) {
    const frame = document.createElement('div');
    frame.className = source.fitCell ? 'sheet-image sheet-image-cell' : 'sheet-image';
    frame.style.gridColumn = source.toCol
      ? `${source.col} / span ${Math.max(1, source.toCol - source.col + 1)}`
      : `${source.col}`;
    frame.style.gridRow = source.toRow
      ? `${source.row} / span ${Math.max(1, source.toRow - source.row + 1)}`
      : `${source.row}`;
    if (!source.fitCell && !source.toCol && source.widthPx) frame.style.width = `${source.widthPx}px`;
    if (!source.fitCell && !source.toRow && source.heightPx) frame.style.height = `${source.heightPx}px`;
    const image = document.createElement('img');
    image.src = source.data;
    image.alt = source.name;
    image.draggable = false;
    frame.appendChild(image);
    grid.appendChild(frame);
  }

  for (const source of sheet.shapes) {
    grid.appendChild(renderShape(source));
  }


  wrap.appendChild(grid);
  section.appendChild(wrap);
  return section;
}

function renderShape(source: ExcelShape): HTMLElement {
  const shape = document.createElement('div');
  shape.className = 'sheet-shape';
  shape.dataset.geometry = source.geometry;
  shape.setAttribute('aria-label', source.name || '形状');
  shape.style.gridColumn = source.toCol
    ? `${source.col} / span ${Math.max(1, source.toCol - source.col + 1)}`
    : `${source.col}`;
  shape.style.gridRow = source.toRow
    ? `${source.row} / span ${Math.max(1, source.toRow - source.row + 1)}`
    : `${source.row}`;
  if (!source.toCol && source.widthPx) shape.style.width = `${source.widthPx}px`;
  if (!source.toRow && source.heightPx) shape.style.height = `${source.heightPx}px`;
  if (source.offsetXPx || source.offsetYPx) {
    shape.style.transform = `translate(${source.offsetXPx}px, ${source.offsetYPx}px)`;
  }
  if (source.fill) shape.style.backgroundColor = source.fill;
  if (source.lineColor || source.lineWidthPx) {
    shape.style.borderColor = source.lineColor ?? '#666';
    shape.style.borderWidth = `${Math.max(1, source.lineWidthPx ?? 1)}px`;
  }
  applyShapeGeometry(shape, source.geometry);
  for (const paragraph of source.paragraphs) {
    const line = document.createElement('div');
    line.className = 'sheet-shape-paragraph';
    renderRichText(line, paragraph.runs);
    shape.appendChild(line);
  }
  return shape;
}

function applyShapeGeometry(shape: HTMLElement, geometry: string): void {
  const clips: Record<string, string> = {
    diamond: 'polygon(50% 0, 100% 50%, 50% 100%, 0 50%)',
    triangle: 'polygon(50% 0, 100% 100%, 0 100%)',
    rtTriangle: 'polygon(0 0, 100% 100%, 0 100%)',
    pentagon: 'polygon(50% 0, 100% 38%, 82% 100%, 18% 100%, 0 38%)',
    hexagon: 'polygon(25% 0, 75% 0, 100% 50%, 75% 100%, 25% 100%, 0 50%)',
    rightArrow: 'polygon(0 25%, 65% 25%, 65% 0, 100% 50%, 65% 100%, 65% 75%, 0 75%)',
    leftArrow: 'polygon(35% 0, 35% 25%, 100% 25%, 100% 75%, 35% 75%, 35% 100%, 0 50%)',
    upArrow: 'polygon(50% 0, 100% 35%, 75% 35%, 75% 100%, 25% 100%, 25% 35%, 0 35%)',
    downArrow: 'polygon(25% 0, 75% 0, 75% 65%, 100% 65%, 50% 100%, 0 65%, 25% 65%)',
    chevron: 'polygon(0 0, 75% 0, 100% 50%, 75% 100%, 0 100%, 25% 50%)',
    parallelogram: 'polygon(25% 0, 100% 0, 75% 100%, 0 100%)',
    trapezoid: 'polygon(20% 0, 80% 0, 100% 100%, 0 100%)',
  };
  if (geometry === 'ellipse') shape.style.borderRadius = '50%';
  else if (geometry === 'roundRect' || geometry.startsWith('round')) shape.style.borderRadius = '8px';
  else if (clips[geometry]) shape.style.clipPath = clips[geometry];
  else if (geometry === 'line' || geometry === 'lineInv') {
    shape.classList.add('sheet-shape-line');
  }
}

function renderRichText(parent: HTMLElement, runs: ExcelRichTextRun[]): void {
  for (const run of runs) {
    const span = document.createElement('span');
    span.textContent = run.text;
    if (run.fontName) span.style.fontFamily = `"${run.fontName}", "PingFang SC", "Microsoft YaHei", sans-serif`;
    if (run.fontSize) span.style.fontSize = `${run.fontSize}pt`;
    if (run.bold != null) span.style.fontWeight = run.bold ? '700' : '400';
    if (run.italic != null) span.style.fontStyle = run.italic ? 'italic' : 'normal';
    if (run.underline != null) span.style.textDecoration = run.underline ? 'underline' : 'none';
    if (run.fontColor) span.style.color = run.fontColor;
    parent.appendChild(span);
  }
}

function applyCellStyle(el: HTMLElement, cell: ExcelCell): void {
  const s: string[] = [];
  if (cell.fontName) el.style.fontFamily = `"${cell.fontName}", "PingFang SC", "Microsoft YaHei", sans-serif`;
  if (cell.fontSize) el.style.fontSize = `${cell.fontSize}pt`;
  if (cell.bold) el.style.fontWeight = '700';
  if (cell.italic) el.style.fontStyle = 'italic';
  if (cell.underline) el.style.textDecoration = 'underline';
  if (cell.fontColor) el.style.color = cell.fontColor;
  if (cell.fill) el.style.background = cell.fill;
  // 对齐作用于格内文本,格子本身始终保持拉伸铺满列(justify-self 会收缩格子导致上下行列宽错位)
  if (cell.hAlign) {
    const h = ALIGN_H[cell.hAlign] ?? 'start';
    if (cell.vAlign === 'Center') el.style.justifyContent = h === 'end' ? 'flex-end' : h === 'center' ? 'center' : 'flex-start';
    else el.style.textAlign = h;
  }
  if (cell.vAlign === 'Center') {
    el.style.display = 'flex';
    el.style.alignItems = 'center';
  }
  if (cell.borderTop) el.style.borderTop = BORDER_MAP[cell.borderTop] ?? BORDER_MAP.thin;
  if (cell.borderLeft) el.style.borderLeft = BORDER_MAP[cell.borderLeft] ?? BORDER_MAP.thin;
  if (cell.borderRight) el.style.borderRight = BORDER_MAP[cell.borderRight] ?? BORDER_MAP.thin;
  if (cell.borderBottom) el.style.borderBottom = BORDER_MAP[cell.borderBottom] ?? BORDER_MAP.thin;
  if (s.length) el.setAttribute('style', (el.getAttribute('style') ?? '') + s.join(';'));
}
