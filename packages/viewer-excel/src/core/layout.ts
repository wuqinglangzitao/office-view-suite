import type { ExcelSheet } from './types';

/** One-based spreadsheet coordinates; end coordinates are inclusive. */
export interface CellRange { c1: number; r1: number; c2: number; r2: number }

/** Parser dimensions may omit empty merged cells and drawing anchors. */
export function sheetBounds(sheet: ExcelSheet): { rows: number; cols: number } {
  let rows = sheet.highestRow, cols = sheet.highestCol;
  const include = (row: number, col: number) => {
    if (!Number.isInteger(row) || row < 0 || row > 1048576
      || !Number.isInteger(col) || col < 0 || col > 16384) {
      throw new Error('Worksheet dimensions exceed Excel limits');
    }
    rows = Math.max(rows, row); cols = Math.max(cols, col);
  };
  include(rows, cols);
  for (const merge of sheet.merges) {
    const range = parseRange(merge);
    include(range.r2, range.c2);
  }
  for (const cell of sheet.cells) include(cell.row, cell.col);
  for (const item of [...sheet.images, ...sheet.shapes]) {
    include(item.row, item.col);
    include(item.toRow ?? item.row, item.toCol ?? item.col);
  }
  return { rows, cols };
}

export function parseRange(value: string): CellRange {
  const address = (part: string) => {
    const match = /^\$?([A-Z]+)\$?([1-9]\d*)$/i.exec(part);
    if (!match) throw new Error(`Invalid cell range: ${value}`);
    let col = 0;
    for (const char of match[1].toUpperCase()) col = col * 26 + char.charCodeAt(0) - 64;
    const row = Number(match[2]);
    if (col > 16384 || row > 1048576) throw new Error(`Cell range exceeds Excel limits: ${value}`);
    return { col, row };
  };
  const parts = value.split(':');
  if (parts.length > 2) throw new Error(`Invalid cell range: ${value}`);
  const a = address(parts[0]), b = address(parts[1] ?? parts[0]);
  return { c1: Math.min(a.col, b.col), r1: Math.min(a.row, b.row), c2: Math.max(a.col, b.col), r2: Math.max(a.row, b.row) };
}

/** Prefix sums provide stable positions and logarithmic viewport lookup. */
export class Axis {
  readonly offsets: Float64Array;
  readonly count: number;
  readonly total: number;

  constructor(sizes: Float64Array) {
    this.count = sizes.length;
    this.offsets = new Float64Array(sizes.length + 1);
    for (let i = 0; i < sizes.length; i++) {
      if (!Number.isFinite(sizes[i])) throw new Error('Worksheet dimensions must be finite');
      this.offsets[i + 1] = this.offsets[i] + Math.max(1, sizes[i]);
    }
    this.total = this.offsets[sizes.length];
  }

  start(index: number): number { return this.offsets[Math.max(0, Math.min(this.count, index - 1))]; }
  end(index: number): number { return this.offsets[Math.max(0, Math.min(this.count, index))]; }
  size(index: number): number { return this.end(index) - this.start(index); }

  at(position: number): number {
    if (this.count === 0) return 0;
    let low = 0, high = this.count;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (this.offsets[middle + 1] <= position) low = middle + 1;
      else high = middle;
    }
    return Math.min(this.count, low + 1);
  }
}

/** Row interval index; large merged rectangles never expand into per-cell sets. */
export class RangeIndex<T extends CellRange> {
  private readonly entries: T[];
  private readonly maxEnd: number[];

  constructor(entries: T[]) {
    this.entries = [...entries].sort((a, b) => a.r1 - b.r1);
    let maximum = 0;
    this.maxEnd = this.entries.map(entry => maximum = Math.max(maximum, entry.r2));
  }

  intersect(query: CellRange): T[] {
    let low = 0, high = this.entries.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (this.entries[middle].r1 <= query.r2) low = middle + 1;
      else high = middle;
    }
    const matches: T[] = [];
    for (let i = low - 1; i >= 0 && this.maxEnd[i] >= query.r1; i--) {
      const entry = this.entries[i];
      if (entry.r2 >= query.r1 && entry.c1 <= query.c2 && entry.c2 >= query.c1) matches.push(entry);
    }
    return matches;
  }
}

export interface AxisWindow { first: number; last: number }

export function visibleWindow(axis: Axis, start: number, length: number, minimum = 1, overscan = 2): AxisWindow {
  if (length <= 0 || axis.count < minimum || start >= axis.total) return { first: 1, last: 0 };
  return {
    first: Math.max(minimum, axis.at(Math.max(0, start)) - overscan),
    last: Math.min(axis.count, axis.at(Math.max(0, start + length - 0.001)) + overscan),
  };
}
