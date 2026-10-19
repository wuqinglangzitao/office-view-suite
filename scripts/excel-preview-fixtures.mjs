// Deterministic DTO fixtures isolate rendering costs from ZIP/WASM parsing.
const cell = (col, row, value, style = {}) => ({ col, row, value, bold: false, italic: false, underline: false, ...style });
const sheet = (name, highestRow, highestCol, cells = []) => ({
  name, highestRow, highestCol, cells, merges: [], colWidths: [], rowHeights: [],
  images: [], shapes: [], frozenColumns: 0, frozenRows: 0,
});

function grid(name, rows, cols) {
  return sheet(name, rows, cols, Array.from({ length: rows * cols }, (_, i) =>
    cell(i % cols + 1, Math.floor(i / cols) + 1, `R${Math.floor(i / cols) + 1}C${i % cols + 1}`)));
}

function sparse(name, rows, cols) {
  const result = sheet(name, rows, cols);
  for (let row = 1; row <= rows; row += 1000) {
    for (let col = 1; col <= Math.min(cols, 20); col++) result.cells.push(cell(col, row, `R${row}C${col}`));
  }
  result.cells.push(cell(cols, rows, 'DOCUMENT-END'));
  return result;
}

export const fixtureNames = ['small', 'multiple-sheets', 'medium', 'sparse-100k', 'wide', 'merged-overlays', 'huge-merge', 'oversized-freeze', 'trailing-overlays'];

export function createFixture(name) {
  switch (name) {
    case 'small': return { sheets: [grid('Small', 40, 12)] };
    case 'multiple-sheets': return { sheets: Array.from({ length: 6 }, (_, i) => grid(`Sheet ${i + 1}`, 120, 25)) };
    case 'medium': return { sheets: [grid('Medium', 1000, 20)] };
    case 'sparse-100k': return { sheets: [sparse('Sparse', 100000, 20)] };
    case 'wide': return { sheets: [sparse('Wide', 200, 5000)] };
    case 'merged-overlays': {
      const value = sparse('Merged', 5000, 20);
      value.frozenRows = 2;
      value.frozenColumns = 2;
      value.merges = ['C10:E80'];
      value.cells.push(cell(3, 10, 'CROSS-VIEWPORT-MERGE', { fill: '#ffff80', wrapText: true }));
      value.cells.push(cell(7, 900, 'A long offscreen line which must not change column width during scrolling'));
      value.cells.push(cell(8, 100, 'line one\nline two\nline three', { wrapText: true }));
      value.colWidths = [{ col: 8, width: 20 }];
      value.images = [{ col: 7, row: 35, toCol: 8, toRow: 38, name: 'fixture-image', fitCell: false,
        data: 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="50" height="30"%3E%3Crect width="50" height="30" fill="red"/%3E%3C/svg%3E' }];
      value.shapes = [{ col: 9, row: 35, toCol: 11, toRow: 38, offsetXPx: 0, offsetYPx: 0,
        name: 'fixture-shape', geometry: 'rect', fill: '#80ff80', paragraphs: [{ runs: [{ text: 'OVERLAY-SHAPE' }] }] }];
      return { sheets: [value] };
    }
    case 'huge-merge': {
      const value = sheet('Huge merge', 100000, 20, [cell(1, 1, 'HUGE-MERGE')]);
      value.merges = ['A1:T100000'];
      return { sheets: [value] };
    }
    case 'oversized-freeze': {
      const value = sparse('Large frozen region', 100000, 20);
      value.frozenRows = 50000;
      value.frozenColumns = 18;
      return { sheets: [value] };
    }
    case 'trailing-overlays': {
      const value = sparse('Trailing drawings', 5000, 20);
      value.images = [{ col: 20, row: 5000, widthPx: 500, heightPx: 1800, fitCell: false, name: 'trailing-image',
        data: 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="500" height="1800"%3E%3Crect width="500" height="1800" fill="red"/%3E%3C/svg%3E' }];
      value.shapes = [{ col: 20, row: 5000, widthPx: 500, heightPx: 100, offsetXPx: 0, offsetYPx: 1700,
        name: 'trailing-shape', geometry: 'rect', fill: '#80ff80', paragraphs: [{ runs: [{ text: 'TRAILING-SHAPE' }] }] }];
      return { sheets: [value] };
    }
    default: throw new Error(`Unknown Excel fixture: ${name}`);
  }
}

export function describeFixture(name) {
  const doc = createFixture(name);
  return {
    name, kind: 'synthetic-DTO-render-only', sheets: doc.sheets.length,
    dimensions: doc.sheets.map(value => ({ rows: value.highestRow, columns: value.highestCol })),
    populatedCells: doc.sheets.reduce((total, value) => total + value.cells.length, 0),
    possibleCells: doc.sheets.reduce((total, value) => total + value.highestRow * value.highestCol, 0),
    merges: doc.sheets.flatMap(value => value.merges),
    style: name === 'merged-overlays' ? 'freeze, merge, wrap, multiline, image, shape' : 'plain text',
  };
}
