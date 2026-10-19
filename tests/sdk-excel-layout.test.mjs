import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';

// Exercise the internal pure geometry module without exposing it as a public SDK API.
const require = createRequire(new URL('../packages/viewer-excel/package.json', import.meta.url));
const ts = require('typescript');
const source = await readFile(new URL('../packages/viewer-excel/src/core/layout.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
const { Axis, RangeIndex, parseRange, sheetBounds, visibleWindow } = await import(`data:text/javascript;base64,${Buffer.from(compiled.outputText).toString('base64')}`);
const layoutUrl = `data:text/javascript;base64,${Buffer.from(compiled.outputText).toString('base64')}`;
const moduleUrl = code => `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`;
const compile = source => ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const formatUrl = moduleUrl(compile(await readFile(new URL('../packages/viewer-excel/src/core/format.ts', import.meta.url), 'utf8')));
const virtualCode = compile(await readFile(new URL('../packages/viewer-excel/src/core/virtual.ts', import.meta.url), 'utf8'))
  .replace("from './format'", `from '${formatUrl}'`).replace("from './layout'", `from '${layoutUrl}'`);
const { renderVirtualSheet } = await import(moduleUrl(virtualCode));

test('Excel dimensions locate variable-size rows and inclusive boundaries', () => {
  const axis = new Axis(new Float64Array([26, 52, 30]));
  assert.equal(axis.total, 108);
  assert.equal(axis.at(0), 1);
  assert.equal(axis.at(25.9), 1);
  assert.equal(axis.at(26), 2);
  assert.equal(axis.at(77.9), 2);
  assert.equal(axis.at(78), 3);
  assert.equal(axis.at(108), 3);
  assert.equal(axis.start(2), 26);
  assert.equal(axis.end(2), 78);
  assert.equal(axis.size(2), 52);
  assert.equal(axis.end(0), 0);
  assert.throws(() => new Axis(new Float64Array([Infinity])), /finite/);
});

test('Excel viewport windows stay independent of total sheet length and exclude frozen rows', () => {
  const axis = new Axis(new Float64Array(100000).fill(26));
  assert.deepEqual(visibleWindow(axis, 52, 520, 3, 2), { first: 3, last: 24 });
  const far = visibleWindow(axis, 2000000, 520, 3, 2);
  assert.ok(far.last - far.first < 26);
  assert.deepEqual(visibleWindow(axis, 0, 0), { first: 1, last: 0 });
  assert.deepEqual(visibleWindow(axis, axis.total, 520), { first: 1, last: 0 });
});

test('Excel merged ranges normalize absolute addresses and reject invalid coordinates', () => {
  assert.deepEqual(parseRange('$C$10:$E$80'), { c1: 3, r1: 10, c2: 5, r2: 80 });
  assert.deepEqual(parseRange('E80:C10'), { c1: 3, r1: 10, c2: 5, r2: 80 });
  assert.deepEqual(parseRange('XFD1048576'), { c1: 16384, r1: 1048576, c2: 16384, r2: 1048576 });
  for (const value of ['A0', 'XFE1', 'A1048577', 'A1:B2:C3', 'oops']) assert.throws(() => parseRange(value));
});

test('Excel interval queries find offscreen anchors without expanding a giant merge', () => {
  const giant = parseRange('A1:XFD1048576');
  const outside = parseRange('A1:B2');
  const visible = parseRange('C90000:E90004');
  const index = new RangeIndex([outside, giant, visible]);
  assert.deepEqual(new Set(index.intersect({ c1: 3, r1: 90002, c2: 20, r2: 90030 })), new Set([giant, visible]));
  assert.deepEqual(index.intersect({ c1: 16385, r1: 1, c2: 16390, r2: 3 }), []);
  assert.deepEqual(new RangeIndex([]).intersect(giant), []);
});

test('Excel sheet bounds include empty merged cells and drawings before choosing a rendering path', () => {
  const sheet = { highestRow: 1, highestCol: 1, merges: ['A1:XFD1048576'], cells: [], images: [], shapes: [] };
  assert.deepEqual(sheetBounds(sheet), { rows: 1048576, cols: 16384 });
  assert.deepEqual(sheetBounds({ ...sheet, merges: [], images: [{ row: 2, col: 3, toRow: 20, toCol: 12 }] }), { rows: 20, cols: 12 });
  assert.throws(() => sheetBounds({ ...sheet, highestRow: Infinity }), /limits/);
  assert.throws(() => sheetBounds({ ...sheet, merges: ['A1:XFE1'] }), /limits/);
});

// Pixel geometry is explicit here; this test does not emulate browser layout.
function viewportHarness(sheet, width, height, onContentChange) {
  const fontListeners = new Set(), frames = new Map();
  let nextFrame = 0;
  const owner = {
    defaultView: {
      getComputedStyle: () => ({ fontFamily: 'sans-serif' }),
      requestAnimationFrame(callback) { frames.set(++nextFrame, callback); return nextFrame; },
      cancelAnimationFrame(id) { frames.delete(id); },
    },
    fonts: { addEventListener(_, listener) { fontListeners.add(listener); }, removeEventListener(_, listener) { fontListeners.delete(listener); } },
    createElement(tag) { return tag === 'canvas' ? { getContext: () => null } : new Element(); },
  };
  class Element {
    ownerDocument = owner;
    style = {};
    dataset = {};
    children = [];
    classes = new Set();
    classList = { add: name => this.classes.add(name), contains: name => this.classes.has(name), toggle: (name, enabled) => enabled ? this.classes.add(name) : this.classes.delete(name) };
    append(child) { this.children.push(child); child.parent = this; }
    remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this); }
    addEventListener() {}
    removeEventListener() {}
  }
  const element = new Element(), viewport = new Element(), grid = new Element();
  Object.assign(viewport, { clientWidth: width, clientHeight: height, scrollLeft: 0, scrollTop: 0 });
  element.append(viewport); viewport.append(grid);
  const rendered = kind => () => { const node = new Element(); node.classList.add(kind); return node; };
  const view = renderVirtualSheet(sheet, element, viewport, grid, {}, { cell: rendered('cell'), image: rendered('image'), shape: rendered('shape') }, undefined, onContentChange);
  const nodes = () => grid.children.flatMap(layer => layer.children.flatMap(pane => pane.children));
  const fontsChanged = () => { for (const listener of fontListeners) listener(); };
  const flushFrame = () => { const pending = [...frames.values()]; frames.clear(); for (const callback of pending) callback(); };
  return { view, viewport, grid, nodes, fontsChanged, flushFrame };
}

test('Excel scrolling includes drawing pixels beyond the final row and column', () => {
  const sheet = {
    highestRow: 1000, highestCol: 20, cells: [], merges: [], rowHeights: [], colWidths: [], frozenRows: 0, frozenColumns: 0,
    images: [{ row: 1000, col: 20, widthPx: 500, heightPx: 900, data: 'data:image/png;base64,', name: 'last-cell', fitCell: false }], shapes: [],
  };
  const { view, viewport, grid, nodes } = viewportHarness(sheet, 300, 100);
  view.resize();
  assert.equal(grid.style.width, '2324px');
  assert.equal(grid.style.height, '26874px');
  assert.equal(view.getContentWidth(), 2325);
  viewport.scrollLeft = 2024;
  viewport.scrollTop = 26774;
  view.resize();
  const image = nodes().find(node => node.classes.has('image'));
  assert.ok(image, 'drawing remains mounted after the viewport passes all cells');
  assert.equal(image.style.left, '-200px');
  assert.equal(image.style.top, '-800px');
  assert.equal(image.style.width, '500px');
  assert.equal(image.style.height, '900px');
  assert.equal(nodes().filter(node => node.classes.has('cell')).length, 0);
  view.dispose();
});

test('Excel shape offsets contribute to scroll bounds and remain visible past the final cell', () => {
  const sheet = {
    highestRow: 1000, highestCol: 20, cells: [], merges: [], rowHeights: [], colWidths: [], frozenRows: 0, frozenColumns: 0, images: [],
    shapes: [{ row: 1000, col: 20, widthPx: 200, heightPx: 150, offsetXPx: 350, offsetYPx: 400, geometry: 'rect', paragraphs: [] }],
  };
  const { view, viewport, grid, nodes } = viewportHarness(sheet, 300, 100);
  view.resize();
  assert.equal(grid.style.width, '2374px');
  assert.equal(grid.style.height, '26524px');
  viewport.scrollLeft = 2074;
  viewport.scrollTop = 26424;
  view.resize();
  const shape = nodes().find(node => node.classes.has('shape'));
  assert.ok(shape);
  assert.equal(shape.style.left, '100px');
  assert.equal(shape.style.top, '-50px');
  view.dispose();
});

test('Excel font remeasurement notifies fit-width after updating geometry without resize recursion', () => {
  const cell = { row: 1, col: 1, value: 'short', fontSize: 10.5 };
  const sheet = { highestRow: 1000, highestCol: 20, cells: [cell], merges: [], rowHeights: [], colWidths: [], frozenRows: 0, frozenColumns: 0, images: [], shapes: [] };
  const widths = [];
  const harness = viewportHarness(sheet, 300, 100, () => { widths.push(harness.view.getContentWidth()); harness.view.resize(); });
  harness.view.resize();
  assert.deepEqual(widths, []);
  const original = harness.view.getContentWidth();
  cell.fontSize = 100; // Change the measurement result to represent a newly loaded font.
  harness.fontsChanged();
  assert.deepEqual(widths, [], 'notification must wait for the measured layout');
  harness.flushFrame();
  assert.equal(widths.length, 1);
  assert.ok(widths[0] > original);
  harness.view.resize();
  assert.equal(widths.length, 1, 'ordinary resize must not notify and recurse');
  harness.view.dispose();
  harness.fontsChanged();
  harness.flushFrame();
  assert.equal(widths.length, 1, 'disposed sheets must release their font listener');
});
