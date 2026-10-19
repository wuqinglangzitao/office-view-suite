import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const require = createRequire(new URL('../packages/viewer/package.json', import.meta.url));
const ts = require('typescript');
const models = { word: 'WordDoc', excel: 'ExcelDoc', ppt: 'PptDoc' };
const engines = new Map();
const documents = new Map();

async function parse(kind, name) {
  if (!engines.has(kind)) engines.set(kind, (async () => {
    const directory = new URL(`../packages/viewer-${kind}/src/wasm/`, import.meta.url);
    const engine = await import(new URL(`${kind}_view_rs.js`, directory).href);
    engine.initSync({ module: await readFile(new URL(`${kind}_view_rs_bg.wasm`, directory)) });
    return engine;
  })());
  const key = `${kind}/${name}`;
  if (!documents.has(key)) documents.set(key, (async () => {
    const engine = await engines.get(kind);
    const source = await readFile(new URL(`../apps/${kind}-view/public/samples/${name}`, import.meta.url));
    return engine.toDocument(source);
  })());
  return documents.get(key);
}

// Type-check the actual serialized output against the renderer's current types.
// No snapshot or second handwritten schema can silently drift with the Rust DTO.
// The generated TypeScript source stays in memory and is never emitted to disk.
function contractDiagnostics(samples) {
  const path = fileURLToPath(new URL('./__wasm_contract__.ts', import.meta.url));
  const source = [
    ...Object.entries(models).map(([kind, type]) => `import type { ${type} } from '../packages/viewer-${kind}/src/core/types';`),
    ...samples.map(({ kind, document }, index) => `const sample${index} = ${JSON.stringify(document)} satisfies ${models[kind]};`),
  ].join('\n');
  const options = {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    types: [],
  };
  const host = ts.createCompilerHost(options);
  const readSource = host.getSourceFile.bind(host);
  host.getSourceFile = (name, languageVersion, ...rest) => name === path
    ? ts.createSourceFile(name, source, languageVersion, true)
    : readSource(name, languageVersion, ...rest);
  return ts.getPreEmitDiagnostics(ts.createProgram([path], options, host));
}

test('real Word, Excel and PPT WASM outputs satisfy their current TypeScript contracts', async () => {
  const fixtures = [
    ['word', 'styles-word.docx'],
    ['excel', 'styles-excel.xlsx'],
    ['excel', 'handmade-merged.xlsx'],
    ['ppt', 'styles-ppt.pptx'],
  ];
  const samples = await Promise.all(fixtures.map(async ([kind, name]) => ({ kind, document: await parse(kind, name) })));
  const diagnostics = contractDiagnostics(samples);
  assert.equal(diagnostics.length, 0, ts.formatDiagnostics(diagnostics, {
    getCanonicalFileName: name => name,
    getCurrentDirectory: () => process.cwd(),
    getNewLine: () => '\n',
  }));
});

test('WASM contract check rejects missing required fields and incompatible nested values', async () => {
  const missing = structuredClone(await parse('excel', 'styles-excel.xlsx'));
  delete missing.sheets[0].frozenRows;
  const changed = structuredClone(await parse('ppt', 'styles-ppt.pptx'));
  changed.slides[0].shapes[0].paragraphs[0].runs[0].bold = 'true';
  const diagnostics = contractDiagnostics([
    { kind: 'excel', document: missing },
    { kind: 'ppt', document: changed },
  ]);
  const messages = diagnostics.map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
  assert.ok(messages.some(message => message.includes('frozenRows')), 'missing Rust fields must fail the TS contract');
  assert.ok(messages.some(message => message.includes('boolean')), 'nested Rust value types must fail the TS contract');
});

test('Word WASM preserves page units, inline styles and merged table structure', async () => {
  const document = await parse('word', 'styles-word.docx');
  assert.deepEqual(document.setup.page, {
    widthPt: 612, heightPt: 792, leftPt: 90, rightPt: 90, topPt: 72, bottomPt: 72, landscape: false,
  });
  const heading = document.blocks.find(block => block.kind === 'heading' && block.level === 1);
  assert.equal(heading.content[0].text, '标题层级 Heading 1');
  assert.equal(heading.content[0].sizeHalfPt, 28);
  assert.equal(heading.content[0].bold, true);
  const table = document.blocks.find(block => block.kind === 'table').table;
  assert.equal(table.grid[1][0].cell.colSpan, 2);
  assert.equal(table.grid[1][1].kind, 'covered');
  assert.equal(table.grid[2][2].cell.rowSpan, 2);
  assert.equal(table.grid[3][2].kind, 'covered');
  assert.equal(document.stats.tables, 1);
  assert.ok(document.stats.paragraphs > 0 && document.stats.runs > 0);
});

test('Excel WASM preserves coordinates, style values, number formats and merge references', async () => {
  const document = await parse('excel', 'styles-excel.xlsx');
  const sheet = document.sheets[0];
  assert.equal(sheet.name, '样式矩阵');
  assert.deepEqual(new Set(sheet.merges), new Set(['A5:C5', 'E5:E7']));
  const cellAt = (col, row) => sheet.cells.find(cell => cell.col === col && cell.row === row);
  const title = cellAt(1, 1);
  assert.equal(title.value, '红色粗体宋体16');
  assert.equal(title.fontName, '宋体');
  assert.equal(title.fontSize, 16);
  assert.equal(title.bold, true);
  assert.equal(title.fontColor, '#CC0000');
  assert.equal(cellAt(3, 2).numfmt, '0.0%');
  assert.equal(cellAt(4, 2).numfmt, 'yyyy-mm-dd');
  assert.equal(sheet.colWidths.find(column => column.col === 1).width, 24);
  assert.equal(sheet.rowHeights.find(row => row.row === 1).height, 30);
  const merged = (await parse('excel', 'handmade-merged.xlsx')).sheets[0];
  assert.equal(merged.cells.find(cell => cell.col === 3 && cell.row === 1).value, '  padded  ');
  assert.deepEqual(new Set(merged.merges), new Set(['A1:B1', 'A2:A3']));
});

test('PPT WASM preserves EMU geometry, text styling and merged table spans', async () => {
  const document = await parse('ppt', 'styles-ppt.pptx');
  assert.equal(document.slideWidthEmu, 12192000);
  assert.equal(document.slideHeightEmu, 6858000);
  assert.equal(document.slides.length, 2);
  const title = document.slides[0].shapes.find(shape => shape.name === 'TextBox 1');
  assert.deepEqual([title.x, title.y, title.w, title.h], [600000, 300000, 10000000, 1200000]);
  const run = title.paragraphs[0].runs[0];
  assert.equal(run.text, '大标题 40pt 红色');
  assert.equal(run.sizePt, 40);
  assert.equal(run.color, '#CC0000');
  assert.equal(run.bold, true);
  const table = document.slides[1].tables[0];
  assert.equal(table.columns.reduce((sum, width) => sum + width, 0), table.w);
  assert.equal(table.rows[1].cells[0].colSpan, 2);
  assert.equal(table.rows[1].cells[0].text, 'r1c0\nr1c1');
  assert.deepEqual(document.warnings, []);
});
