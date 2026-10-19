import { formatValue } from './format';
import { Axis, parseRange, RangeIndex, visibleWindow, type CellRange } from './layout';
import type { ExcelCell, ExcelImage, ExcelRichTextRun, ExcelShape, ExcelSheet } from './types';

export const VIRTUAL_CELL_THRESHOLD = 10000;
// Unusually tiny source dimensions must fail visibly instead of freezing the page.
const MAX_VIEWPORT_NODES = 20000;

interface Overlay extends CellRange {
  // The interval index uses document pixels for drawings, not cell addresses.
  key: string;
  x: number; y: number; width: number; height: number;
  image?: ExcelImage;
  shape?: ExcelShape;
}

interface SheetLayout {
  columns: Axis;
  rows: Axis;
  width: number;
  height: number;
  merges: RangeIndex<CellRange>;
  cells: Map<string, ExcelCell>;
  overlays: RangeIndex<Overlay>;
}

export interface VirtualCache { layout?: SheetLayout }
export interface SheetView {
  element: HTMLElement;
  viewport: HTMLElement;
  resize(): void;
  getContentWidth(): number;
  dispose(): void;
}
interface Renderers {
  cell(cell: ExcelCell | undefined, row: number, col: number): HTMLElement;
  image(source: ExcelImage): HTMLElement;
  shape(source: ExcelShape): HTMLElement;
}

function sheetLayout(sheet: ExcelSheet, owner: Document, family: string): SheetLayout {
  if (!Number.isInteger(sheet.highestRow) || sheet.highestRow < 0 || sheet.highestRow > 1048576
    || !Number.isInteger(sheet.highestCol) || sheet.highestCol < 0 || sheet.highestCol > 16384) {
    throw new Error('Worksheet dimensions exceed Excel limits');
  }
  const colSizes = new Float64Array(sheet.highestCol).fill(96);
  const rowSizes = new Float64Array(sheet.highestRow).fill(26);
  const explicitCols = new Set<number>(), explicitRows = new Set<number>();
  for (const dim of sheet.colWidths) if (dim.col >= 1 && dim.col <= colSizes.length && dim.width > 0) {
    colSizes[dim.col - 1] = Math.max(1, Math.round(dim.width * 7 + 5));
    explicitCols.add(dim.col);
  }
  for (const dim of sheet.rowHeights) if (dim.row >= 1 && dim.row <= rowSizes.length && dim.height > 0) {
    rowSizes[dim.row - 1] = Math.max(1, Math.round(dim.height * 96 / 72));
    explicitRows.add(dim.row);
  }
  const ranges = sheet.merges.map(parseRange);
  const anchors = new Map(ranges.map(range => [`${range.c1}:${range.r1}`, range]));
  const cells = new Map(sheet.cells.map(cell => [`${cell.col}:${cell.row}`, cell]));
  const wrapCols = new Set(sheet.cells.filter(cell => cell.wrapText).map(cell => cell.col));
  const context = owner.createElement('canvas').getContext('2d');
  const font = (cell: ExcelCell, run?: ExcelRichTextRun) => {
    const size = (run?.fontSize ?? cell.fontSize ?? 10.5) * 96 / 72;
    const name = run?.fontName ?? cell.fontName;
    if (context) context.font = `${run?.italic ?? cell.italic ? 'italic ' : ''}${run?.bold ?? cell.bold ? 'bold ' : ''}${size}px ${name ? `"${name}", ` : ''}${family}`;
    return size;
  };
  const runs = (cell: ExcelCell): ExcelRichTextRun[] => cell.richText?.length ? cell.richText : [{ text: formatValue(cell.value, cell.numfmt) }];
  const width = (text: string, size: number) => context?.measureText(text).width ?? text.length * size;
  // Only implicit non-wrapping columns grow with content. The result is fixed for all scroll positions.
  for (const cell of sheet.cells) {
    if (cell.col < 1 || cell.col > colSizes.length || explicitCols.has(cell.col) || wrapCols.has(cell.col)) continue;
    if (anchors.has(`${cell.col}:${cell.row}`)) continue;
    let measured = 0;
    for (const run of runs(cell)) measured += width(run.text.replace(/\s+/g, ' '), font(cell, run));
    colSizes[cell.col - 1] = Math.max(colSizes[cell.col - 1], Math.ceil(measured + 17));
  }
  const columns = new Axis(colSizes);
  for (const cell of sheet.cells) {
    if (cell.row < 1 || cell.row > rowSizes.length || explicitRows.has(cell.row)) continue;
    const merge = anchors.get(`${cell.col}:${cell.row}`);
    // Multi-row merges retain source row heights; a single cell must not expand every spanned row.
    if (merge && merge.r2 > merge.r1) continue;
    const available = Math.max(1, columns.end(merge?.c2 ?? cell.col) - columns.start(cell.col) - 17);
    let lines = 1, used = 0, tallest = 14;
    for (const run of runs(cell)) {
      const size = font(cell, run);
      tallest = Math.max(tallest, size);
      if (!cell.wrapText) continue;
      // Respect word boundaries and East Asian wrapping; long unbreakable words stay on one line.
      const tokens = run.text.match(/[\u2e80-\u9fff]|[^\s\u2e80-\u9fff]+|\s+/g) ?? [];
      for (const token of tokens) {
        const space = /^\s+$/.test(token);
        const measured = width(space ? ' ' : token, size);
        if (!space && used > 0 && used + measured > available) { lines++; used = 0; }
        if (!space || used > 0) used += measured;
      }
    }
    rowSizes[cell.row - 1] = Math.max(rowSizes[cell.row - 1], Math.ceil(lines * tallest * 1.25 + 7));
  }
  const rows = new Axis(rowSizes);
  const overlay = (source: ExcelImage | ExcelShape, key: string): Overlay => {
    const x = columns.start(source.col) + ('offsetXPx' in source ? source.offsetXPx : 0);
    const y = rows.start(source.row) + ('offsetYPx' in source ? source.offsetYPx : 0);
    const w = source.toCol ? columns.end(source.toCol) - columns.start(source.col) : source.widthPx ?? columns.size(source.col);
    const h = source.toRow ? rows.end(source.toRow) - rows.start(source.row) : source.heightPx ?? rows.size(source.row);
    return { key, x, y, width: Math.max(1, w), height: Math.max(1, h),
      c1: x, r1: y, c2: x + Math.max(1, w) - 0.001, r2: y + Math.max(1, h) - 0.001,
      ...('data' in source ? { image: source } : { shape: source }),
    };
  };
  const drawings = [
    ...sheet.images.map((image, i) => overlay(image, `image:${i}`)),
    ...sheet.shapes.map((shape, i) => overlay(shape, `shape:${i}`)),
  ];
  let widthPx = columns.total, heightPx = rows.total;
  for (const drawing of drawings) {
    widthPx = Math.max(widthPx, drawing.x + drawing.width);
    heightPx = Math.max(heightPx, drawing.y + drawing.height);
  }
  return { columns, rows, width: widthPx, height: heightPx, cells, merges: new RangeIndex(ranges), overlays: new RangeIndex(drawings) };
}

interface Pane {
  element: HTMLElement;
  nodes: Map<string, HTMLElement>;
  frozenX: boolean;
  frozenY: boolean;
}

export function renderVirtualSheet(sheet: ExcelSheet, element: HTMLElement, viewport: HTMLElement, grid: HTMLElement, cache: VirtualCache, renderers: Renderers, onError?: (error: Error) => void, onContentChange?: () => void): SheetView {
  grid.classList.add('virtual-grid');
  element.dataset.virtual = 'true';
  const owner = element.ownerDocument;
  const win = owner.defaultView!;
  const layer = owner.createElement('div');
  layer.className = 'virtual-window';
  grid.append(layer);
  const panes: Pane[] = [[false, false], [true, false], [false, true], [true, true]].map(([frozenX, frozenY]) => {
    const pane = owner.createElement('div');
    pane.className = 'virtual-pane';
    layer.append(pane);
    return { element: pane, nodes: new Map(), frozenX, frozenY };
  });
  let disposed = false;
  let failed = false;
  let frame: number | undefined;
  let notifyContentChange = false;
  let layout: SheetLayout | undefined = cache.layout;
  let lastWidth = 0, lastHeight = 0;

  const update = () => {
    frame = undefined;
    if (disposed || viewport.clientWidth <= 0 || viewport.clientHeight <= 0) return;
    if (!layout) {
      layout = sheetLayout(sheet, owner, win.getComputedStyle(element).fontFamily || 'sans-serif');
      cache.layout = layout;
    }
    const { columns, rows, cells, merges, overlays } = layout;
    grid.style.width = `${layout.width}px`;
    grid.style.height = `${layout.height}px`;
    // All dimensions are local CSS pixels, including when an ancestor uses CSS zoom.
    const w = viewport.clientWidth, h = viewport.clientHeight;
    lastWidth = w; lastHeight = h;
    const sx = viewport.scrollLeft, sy = viewport.scrollTop;
    layer.style.left = `${sx}px`; layer.style.top = `${sy}px`;
    layer.style.width = `${w}px`; layer.style.height = `${h}px`;
    const frozenColumns = Math.max(0, Math.min(columns.count, sheet.frozenColumns));
    const frozenRows = Math.max(0, Math.min(rows.count, sheet.frozenRows));
    const fw = Math.min(w, columns.end(frozenColumns)), fh = Math.min(h, rows.end(frozenRows));
    let nodeCount = 0;
    for (const pane of panes) {
      const px = pane.frozenX ? 0 : fw, py = pane.frozenY ? 0 : fh;
      const pw = pane.frozenX ? fw : w - fw, ph = pane.frozenY ? fh : h - fh;
      pane.element.style.left = `${px}px`; pane.element.style.top = `${py}px`;
      pane.element.style.width = `${pw}px`; pane.element.style.height = `${ph}px`;
      const ox = pane.frozenX ? 0 : sx + fw, oy = pane.frozenY ? 0 : sy + fh;
      const cx = visibleWindow(columns, ox, pw, pane.frozenX ? 1 : frozenColumns + 1);
      const ry = visibleWindow(rows, oy, ph, pane.frozenY ? 1 : frozenRows + 1);
      if (pane.frozenX) cx.last = Math.min(cx.last, frozenColumns);
      if (pane.frozenY) ry.last = Math.min(ry.last, frozenRows);
      const needed = new Set<string>();
      const place = (key: string, create: () => HTMLElement, x: number, y: number, width: number, height: number) => {
        if (++nodeCount > MAX_VIEWPORT_NODES) throw new Error(`Worksheet viewport exceeds the preview limit of ${MAX_VIEWPORT_NODES} visible cells and drawings`);
        needed.add(key);
        let node = pane.nodes.get(key);
        if (!node) {
          node = create();
          if (node.classList.contains('cell')) {
            node.classList.toggle('frozen-column', pane.frozenX);
            node.classList.toggle('frozen-row', pane.frozenY);
            node.classList.toggle('frozen-corner', pane.frozenX && pane.frozenY);
          }
          pane.nodes.set(key, node); pane.element.append(node);
        }
        node.style.left = `${x - ox}px`; node.style.top = `${y - oy}px`;
        node.style.width = `${width}px`; node.style.height = `${height}px`;
      };
      if (pw > 0 && ph > 0 && cx.last >= cx.first && ry.last >= ry.first) {
        const range = { c1: cx.first, c2: cx.last, r1: ry.first, r2: ry.last };
        const spanning = merges.intersect(range);
        const intervals = new Map<number, CellRange[]>();
        for (const merged of spanning) {
          for (let r = Math.max(ry.first, merged.r1); r <= Math.min(ry.last, merged.r2); r++) {
            const list = intervals.get(r) ?? [];
            list.push(merged); intervals.set(r, list);
          }
          place(`merge:${merged.c1}:${merged.r1}`, () => renderers.cell(cells.get(`${merged.c1}:${merged.r1}`), merged.r1, merged.c1),
            columns.start(merged.c1), rows.start(merged.r1), columns.end(merged.c2) - columns.start(merged.c1), rows.end(merged.r2) - rows.start(merged.r1));
        }
        for (let r = ry.first; r <= ry.last; r++) {
          const covered = (intervals.get(r) ?? []).sort((a, b) => a.c1 - b.c1);
          let mergeIndex = 0;
          for (let c = cx.first; c <= cx.last; c++) {
            while (mergeIndex < covered.length && covered[mergeIndex].c2 < c) mergeIndex++;
            if (covered[mergeIndex]?.c1 <= c) { c = covered[mergeIndex].c2; continue; }
            place(`cell:${c}:${r}`, () => renderers.cell(cells.get(`${c}:${r}`), r, c), columns.start(c), rows.start(r), columns.size(c), rows.size(r));
          }
        }
      }
      if (pw > 0 && ph > 0) {
        for (const item of overlays.intersect({ c1: ox, r1: oy, c2: ox + pw - 0.001, r2: oy + ph - 0.001 })) {
          place(item.key, () => {
            const node = item.image ? renderers.image(item.image) : renderers.shape(item.shape!);
            node.style.transform = '';
            return node;
          }, item.x, item.shape?.geometry === 'line' || item.shape?.geometry === 'lineInv' ? item.y + item.height / 2 : item.y, item.width, item.height);
        }
      }
      for (const [key, node] of pane.nodes) if (!needed.has(key)) { node.remove(); pane.nodes.delete(key); }
    }
    if (notifyContentChange) {
      notifyContentChange = false;
      onContentChange?.();
    }
  };
  const schedule = () => {
    if (!disposed && !failed && frame === undefined) frame = win.requestAnimationFrame(() => {
      try { update(); }
      catch (error) {
        failed = true;
        if (onError) onError(error instanceof Error ? error : new Error(String(error)));
        else throw error;
      }
    });
  };
  viewport.addEventListener('scroll', schedule, { passive: true });
  const observer = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => {
    if (viewport.clientWidth !== lastWidth || viewport.clientHeight !== lastHeight) schedule();
  });
  observer?.observe(viewport);
  const fontsChanged = () => { layout = undefined; cache.layout = undefined; notifyContentChange = true; schedule(); };
  owner.fonts?.addEventListener('loadingdone', fontsChanged);
  return {
    element, viewport,
    resize() {
      if (!layout && !disposed) {
        layout = sheetLayout(sheet, owner, win.getComputedStyle(element).fontFamily || 'sans-serif');
        cache.layout = layout;
      }
      if (layout) { grid.style.width = `${layout.width}px`; grid.style.height = `${layout.height}px`; }
      if (frame !== undefined) { win.cancelAnimationFrame(frame); frame = undefined; }
      update();
    },
    getContentWidth() { return (layout?.width ?? 0) + 1; },
    dispose() {
      disposed = true;
      if (frame !== undefined) win.cancelAnimationFrame(frame);
      observer?.disconnect();
      viewport.removeEventListener('scroll', schedule);
      owner.fonts?.removeEventListener('loadingdone', fontsChanged);
      for (const pane of panes) pane.nodes.clear();
      element.remove();
    },
  };
}
