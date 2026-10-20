import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { collectWordFlowContent } from './word-flow-content.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const built = await build({ configFile: false, logLevel: 'silent', build: {
  write: false, minify: false,
  lib: { entry: resolve(root, 'packages/viewer-word/src/core/render.ts'), name: 'wordDisplay', formats: ['iife'] },
} });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const results = [];
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 1000 } });
  await page.setContent('<div id="host" style="width:900px"></div>');
  await page.addScriptTag({ content: built[0].output.find(output => output.type === 'chunk').code });
  await page.addScriptTag({ content: `window.collectWordFlowContent = ${collectWordFlowContent.toString()};` });
  const observed = await page.evaluate(async () => {
    const host = document.querySelector('#host');
    let rendered;
    const setup = { page: { widthPt: 612, heightPt: 792, leftPt: 72, rightPt: 72, topPt: 72, bottomPt: 72 },
      header: [], firstHeader: [], evenHeader: [], footer: [], firstFooter: [], evenFooter: [], titlePage: false, pageNumbers: false };
    const p = (text, rest = {}) => ({ kind: 'paragraph', content: [{ kind: 'text', text }], ...rest });
    const render = (blocks, settings = setup) => {
      rendered?.dispose();
      rendered = wordDisplay.renderWordInto(host, { blocks, setup: settings, stats: { paragraphs: 0, tables: 0, images: 0, runs: 0 } });
      return host.shadowRoot;
    };
    const list = { kind: 'listItem', list: { level: 0, ordered: false, label: '•', markerFont: 'Symbol' }, content: [{ kind: 'text', text: 'Table list' }] };
    let shadow = render([{ kind: 'table', table: { headerRows: 0, colWidthsPct: [1], fixedLayout: true, bidiVisual: true,
      grid: [[{ kind: 'origin', cell: { colSpan: 1, rowSpan: 1, blocks: [list] } }]] } }]);
    const lists = { marker: shadow.querySelector('td li')?.dataset.marker, tableDirection: getComputedStyle(shadow.querySelector('table')).direction };
    shadow = render([p('capitalized', { shading: '#123456', bidi: true, content: [{ kind: 'text', text: 'capitalized', caps: true, smallCaps: true, rtl: true }] })]);
    const paragraph = shadow.querySelector('.page-body p');
    const run = paragraph.querySelector('span');
    const styles = { background: getComputedStyle(paragraph).backgroundColor, direction: getComputedStyle(paragraph).direction,
      transform: getComputedStyle(run).textTransform, caps: getComputedStyle(run).fontVariantCaps, runDirection: getComputedStyle(run).direction };
    const image = { kind: 'image', src: 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>', widthPx: 10, heightPx: 10,
      rotationDegrees: 90, anchor: { horizontalFrom: 'page', verticalFrom: 'page', xPt: 100, yPt: 110 } };
    shadow = render([p('', { content: [image, { kind: 'text', text: 'Anchored image' }] })]);
    const img = shadow.querySelector('.page-body img');
    const imageStyle = getComputedStyle(img);
    const imageBounds = img.getBoundingClientRect();
    const pageBounds = shadow.querySelector('.page').getBoundingClientRect();
    const anchors = { position: imageStyle.position, transform: imageStyle.transform,
      x: imageBounds.x - pageBounds.x, y: imageBounds.y - pageBounds.y };
    const pageField = p('', { content: [{ kind: 'text', text: 'cached', field: { kind: 'PAGE' } }] });
    shadow = render([p('first'), { kind: 'pageBreak' }, p('second')], { ...setup, footer: [pageField], pageNumberStart: 4, pageNumberFormat: 'lowerRoman' });
    const fields = [...shadow.querySelectorAll('.page-footer')].map(node => node.textContent);
    shadow = render([p('column one'), ...Array.from({ length: 35 }, (_, index) => p('Row ' + index))], { ...setup, columns: 2, columnGapPt: 24 });
    const columns = { count: shadow.querySelectorAll('.page-column').length,
      pages: shadow.querySelectorAll('.page').length };
    shadow = render([p('甲\n乙', { columnBreakOffsets: [1] })], { ...setup, columns: 2 });
    const columnBreak = [...shadow.querySelectorAll('.page-column')].map(column => column.textContent);
    const unevenSetup = { ...setup, columns: 2, columnWidthsPt: [160, 280], columnSpacesPt: [28] };
    shadow = render([p('narrow'), { kind: 'columnBreak' }, p('wide')], unevenSetup);
    const unequalColumns = [...shadow.querySelectorAll('.page-column')].map(column => ({
      width: column.getBoundingClientRect().width, gap: parseFloat(getComputedStyle(column).marginInlineEnd), text: column.textContent,
    }));
    const geometryChanges = [
      { columnWidthsPt: [200, 240] }, { columnSpacesPt: [18] }, { columnGapPt: 12 },
      { columnsRtl: true }, { columnSeparator: true }, { textDirection: 'topToBottomRightToLeft' }, { linePitchPt: 22 },
    ].map(change => {
      shadow = render([p('previous'), { kind: 'sectionBreak', section: { ...unevenSetup, breakType: 'continuous' } }, p('next')], { ...unevenSetup, ...change });
      return { change, pages: shadow.querySelectorAll('.page').length };
    });
    shadow = render([p('before section'), { kind: 'sectionBreak', section: {
      ...setup, columns: 2, breakType: 'nextColumn', header: [p('old header')], footer: [pageField],
    } }, p('after section'), { kind: 'pageBreak' }, p('next page')], {
      ...setup, columns: 2, header: [p('new header')], footer: [pageField],
    });
    const sharedSection = [...shadow.querySelectorAll('.page')].map(page => ({
      header: page.querySelector('.page-header').textContent, footer: page.querySelector('.page-footer').textContent,
      columns: [...page.querySelectorAll('.page-column')].map(column => column.textContent),
    }));
    const longText = Array.from({ length: 180 }, (_, index) => `word${index}`).join(' ');
    const rowDocument = cantSplit => [{ kind: 'table', table: { headerRows: 0, colWidthsPct: [1], fixedLayout: true,
      rowCantSplit: [cantSplit], grid: [[{ kind: 'origin', cell: { colSpan: 1, rowSpan: 1, blocks: [p(longText, { pagination: { keepNext: false, keepLines: false, widowControl: false } })] } }]] } }];
    const smallSetup = { ...setup, page: { widthPt: 320, heightPt: 260, leftPt: 25, rightPt: 25, topPt: 25, bottomPt: 25 } };
    shadow = render(rowDocument(false), smallSetup);
    const splitRow = { pages: shadow.querySelectorAll('.page').length, overflow: shadow.querySelectorAll('.oversize-block').length,
      text: [...shadow.querySelectorAll('td')].map(cell => cell.textContent).join('') };
    shadow = render(rowDocument(true), smallSetup);
    const keptRow = { pages: shadow.querySelectorAll('.page').length, overflow: shadow.querySelectorAll('.oversize-block').length };
    shadow = render([{ kind: 'table', table: { headerRows: 1, colWidthsPct: [0.15, 0.7, 0.15], fixedLayout: true,
      grid: [['Left header', 'Detail header', 'Right header'], ['Before', longText, 'After']].map(row => row.map(value => ({
        kind: 'origin', cell: { colSpan: 1, rowSpan: 1, blocks: [p(value, { pagination: { keepNext: false, keepLines: false, widowControl: false } })] },
      }))) } }], smallSetup);
    const multiCellRow = { pages: shadow.querySelectorAll('.page').length, ...collectWordFlowContent(shadow) };
    shadow = render(Array.from({ length: 30 }, () => p('竖排文字的分页与横排使用不同的边界')), { ...smallSetup, textDirection: 'topToBottomRightToLeft' });
    const vertical = { mode: getComputedStyle(shadow.querySelector('.page-body')).writingMode, pages: shadow.querySelectorAll('.page').length,
      paragraphs: shadow.querySelectorAll('.page-body p').length };
    rendered.dispose();
    return { lists, styles, anchors, fields, columns, columnBreak, unequalColumns, geometryChanges, sharedSection, splitRow, keptRow, multiCellRow, vertical, longText };
  });
  const checks = [
    ['table-list-markers-and-rtl', () => assert.deepEqual(observed.lists, { marker: '•', tableDirection: 'rtl' })],
    ['source-paragraph-and-run-styles', () => assert.deepEqual(observed.styles, { background: 'rgb(18, 52, 86)', direction: 'rtl', transform: 'uppercase', caps: 'small-caps', runDirection: 'rtl' })],
    ['body-image-anchor-and-rotation', () => { assert.equal(observed.anchors.position, 'absolute'); assert.notEqual(observed.anchors.transform, 'none'); assert.ok(Math.abs(observed.anchors.x - 100 * 4 / 3) < 1); assert.ok(Math.abs(observed.anchors.y - 110 * 4 / 3) < 1); }],
    ['dynamic-section-page-numbers', () => assert.deepEqual(observed.fields, ['iv', 'v'])],
    ['two-column-page-flow', () => { assert.ok(observed.columns.count >= 2); assert.equal(observed.columns.pages, 1); }],
    ['explicit-column-break', () => assert.deepEqual(observed.columnBreak, ['甲', '乙'])],
    ['unequal-columns-keep-source-width-and-gap', () => {
      assert.deepEqual(observed.unequalColumns.map(column => column.text), ['narrow', 'wide']);
      assert.ok(Math.abs(observed.unequalColumns[0].width - 160 * 4 / 3) < 1);
      assert.ok(Math.abs(observed.unequalColumns[1].width - 280 * 4 / 3) < 1);
      assert.ok(Math.abs(observed.unequalColumns[0].gap - 28 * 4 / 3) < 1);
    }],
    ['continuous-sections-rebuild-changed-column-geometry', () => {
      for (const entry of observed.geometryChanges) assert.equal(entry.pages, 2, JSON.stringify(entry.change));
    }],
    ['next-column-shares-page-header-and-updates-following-page', () => assert.deepEqual(observed.sharedSection, [
      { header: 'old header', footer: '1', columns: ['before section', 'after section'] },
      { header: 'new header', footer: '2', columns: ['next page', ''] },
    ])],
    ['splittable-table-row-preserves-all-content', () => { assert.ok(observed.splitRow.pages > 1); assert.equal(observed.splitRow.overflow, 0); assert.equal(observed.splitRow.text, observed.longText); }],
    ['cant-split-row-retains-bounded-fallback', () => assert.deepEqual(observed.keptRow, { pages: 1, overflow: 1 })],
    ['split-cells-preserve-source-order-and-repeat-headers-once-logically', () => {
      assert.ok(observed.multiCellRow.pages > 1);
      assert.deepEqual(observed.multiCellRow.cells, ['Left header', 'Detail header', 'Right header', 'Before', observed.longText, 'After']);
    }],
    ['vertical-section-pagination', () => { assert.equal(observed.vertical.mode, 'vertical-rl'); assert.ok(observed.vertical.pages > 1); assert.equal(observed.vertical.paragraphs, 30); }],
  ];
  for (const [name, check] of checks) {
    try { check(); results.push({ name, passed: true }); }
    catch (error) { results.push({ name, passed: false, error: error.message }); }
  }
  console.log(JSON.stringify({ observed, results }, null, 2));
  assert.ok(results.every(result => result.passed), 'Word display regression failures');
} finally { await browser.close(); }
