import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { collectWordFlowContent } from './word-flow-content.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output/playwright/word-tables');
const harness = await realpath(await mkdtemp(resolve(tmpdir(), 'word-tables-')));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const wasm = resolve(root, 'packages/viewer-word/src/wasm');
const engine = await import(pathToFileURL(resolve(wasm, 'word_view_rs.js')).href);
engine.initSync({ module: await readFile(resolve(wasm, 'word_view_rs_bg.wasm')) });
await mkdir(output, { recursive: true });

const w = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const a = 'http://schemas.openxmlformats.org/drawingml/2006/main';
const p = text => `<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="20"/></w:rPr><w:t>${text}</w:t></w:r></w:p>`;
const cell = (text, props = '') => `<w:tc><w:tcPr>${props}</w:tcPr>${p(text)}</w:tc>`;
const row = (cells, props = '', exceptions = '') => `<w:tr>${exceptions ? `<w:tblPrEx>${exceptions}</w:tblPrEx>` : ''}<w:trPr>${props}</w:trPr>${cells.join('')}</w:tr>`;
const table = (rows, props = '', widths = [2400, 2400]) => `<w:tbl><w:tblPr>${props}<w:tblW w:w="5000" w:type="pct"/><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid>${widths.map(width => `<w:gridCol w:w="${width}"/>`).join('')}</w:tblGrid>${rows.join('')}</w:tbl>`;
const borders = (style = 'single', size = 8, color = '555555') => `<w:tblBorders>${['top', 'left', 'bottom', 'right', 'insideH', 'insideV'].map(side => `<w:${side} w:val="${style}" w:sz="${size}" w:color="${color}"/>`).join('')}</w:tblBorders>`;
const styles = `<w:styles xmlns:w="${w}">
  <w:style w:type="table" w:styleId="Base"><w:tblPr>${borders()}</w:tblPr></w:style>
  <w:style w:type="table" w:styleId="Derived"><w:basedOn w:val="Base"/>
    <w:tblPr><w:tblStyleRowBandSize w:val="1"/></w:tblPr>
    <w:tblStylePr w:type="firstRow"><w:tcPr><w:shd w:val="clear" w:fill="2F5496"/><w:tcBorders><w:insideV w:val="dotted" w:sz="8" w:color="123456"/></w:tcBorders></w:tcPr></w:tblStylePr>
    <w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:fill="EAF1FB"/></w:tcPr></w:tblStylePr>
  </w:style>
  <w:style w:type="table" w:styleId="Theme"><w:basedOn w:val="Base"/>
    <w:tblStylePr w:type="firstRow"><w:tcPr><w:shd w:val="clear" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillTint="80"/></w:tcPr></w:tblStylePr>
  </w:style>
  <w:style w:type="table" w:styleId="UnsizedHeader">
    <w:tblStylePr w:type="firstRow"><w:tcPr><w:tcBorders><w:bottom w:val="single"/></w:tcBorders></w:tcPr></w:tblStylePr>
  </w:style>
</w:styles>`;

async function source(name, body, colorMapping = '') {
  const directory = resolve(harness, name);
  await mkdir(resolve(directory, 'word/_rels'), { recursive: true });
  await mkdir(resolve(directory, 'word/theme'), { recursive: true });
  await mkdir(resolve(directory, '_rels'), { recursive: true });
  const parts = {
    '[Content_Types].xml': `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>`,
    '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
    'word/_rels/document.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>',
    'word/styles.xml': styles,
    'word/theme/theme1.xml': `<a:theme xmlns:a="${a}" name="Regression"><a:themeElements><a:clrScheme name="Regression"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:accent1><a:srgbClr val="204060"/></a:accent1><a:accent2><a:srgbClr val="A0B0C0"/></a:accent2></a:clrScheme></a:themeElements></a:theme>`,
    'word/settings.xml': `<w:settings xmlns:w="${w}">${colorMapping}</w:settings>`,
    'word/document.xml': `<w:document xmlns:w="${w}"><w:body>${body}<w:sectPr><w:pgSz w:w="6400" w:h="4400"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr></w:body></w:document>`,
  };
  for (const [name, value] of Object.entries(parts)) await writeFile(resolve(directory, name), value);
  const archive = resolve(harness, `${name}.docx`);
  execFileSync('zip', ['-q', '-r', archive, '.'], { cwd: directory });
  return engine.toDocument(await readFile(archive));
}

await writeFile(resolve(harness, 'index.html'), '<!doctype html><meta charset="utf-8"><title>Word table regression</title><style>body{margin:0;background:#ddd}#host{width:700px}</style><div id="host"></div><script type="module" src="/main.js"></script>');
await writeFile(resolve(harness, 'main.js'), `
import {renderWordInto} from ${JSON.stringify(resolve(root, 'packages/viewer-word/src/core/render.ts'))};
const host=document.querySelector('#host');let rendered;
window.wordTables={async render(doc,zoom=1){
  rendered?.dispose();host.style.zoom=String(zoom);await document.fonts.ready;
  const idle=window.requestIdleCallback,cancel=window.cancelIdleCallback,pending=new Map();let next=0;
  window.requestIdleCallback=fn=>{pending.set(++next,fn);return next;};window.cancelIdleCallback=id=>pending.delete(id);
  try{
    rendered=renderWordInto(host,doc);let batches=0;
    while(pending.size){if(++batches>1000)throw new Error('Pagination did not settle');const [id,fn]=pending.entries().next().value;pending.delete(id);fn();}
    return {pages:host.shadowRoot.querySelectorAll('.page').length,cells:Array.from(host.shadowRoot.querySelectorAll('td,th'),cell=>{
      const style=getComputedStyle(cell),edges={};
      for(const side of ['Top','Right','Bottom','Left'])edges[side.toLowerCase()]={style:style['border'+side+'Style'],color:style['border'+side+'Color'],width:style['border'+side+'Width']};
      return {text:cell.textContent,tag:cell.tagName,background:style.backgroundColor,colSpan:cell.colSpan,rowSpan:cell.rowSpan,edges};
    })};
  }finally{window.requestIdleCallback=idle;window.cancelIdleCallback=cancel;}
}};
`);
await build({ root: harness, configFile: false, base: '/word-tables/', logLevel: 'warn', build: { target: 'es2022' } });
const dist = resolve(harness, 'dist');
const server = createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  const path = resolve(dist, decodeURIComponent(url.pathname.slice('/word-tables/'.length)) || 'index.html');
  if (!url.pathname.startsWith('/word-tables/') || !path.startsWith(dist + sep)) { response.writeHead(403).end(); return; }
  try { response.writeHead(200, { 'Content-Type': { '.html': 'text/html', '.js': 'text/javascript' }[extname(path)] || 'application/octet-stream' }).end(await readFile(path)); }
  catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 800 }, deviceScaleFactor: 1 });
const results = [], errors = [];
page.on('pageerror', error => errors.push(error.message));
const render = (doc, zoom = 1) => page.evaluate(({ doc, zoom }) => window.wordTables.render(doc, zoom), { doc, zoom });
const find = (result, text) => { const found = result.cells.find(cell => cell.text === text); assert.ok(found, `Missing cell: ${text}`); return found; };
const dtoCell = (doc, text) => doc.blocks.filter(block => block.table).flatMap(block => block.table.grid.flat()).find(slot => slot.cell?.blocks.some(block => block.content?.some(run => run.text === text)))?.cell;
const check = async (name, run) => {
  try { const details = await run(); results.push({ name, passed: true, ...details }); console.log(`PASS ${name}`); }
  catch (error) { results.push({ name, passed: false, error: error.message }); console.error(`FAIL ${name}: ${error.message}`); }
};
try {
  await page.goto(`http://127.0.0.1:${server.address().port}/word-tables/`);
  await page.waitForFunction(() => !!window.wordTables);
  if (process.env.WORD_TABLE_DOCX) await check('external-document-table-content-is-visible', async () => {
    const doc = engine.toDocument(await readFile(resolve(process.env.WORD_TABLE_DOCX)));
    const sourceTables = doc.blocks.filter(block => block.table).map(block => block.table);
    if (process.env.WORD_TABLE_EXPECT_MIN_ROW_HEIGHTS) {
      const expected = JSON.parse(process.env.WORD_TABLE_EXPECT_MIN_ROW_HEIGHTS);
      assert.equal(sourceTables.length, expected.length, 'Source table count must match');
      expected.forEach((heights, index) => {
        const actual = sourceTables[index].rowMinHeightsPt;
        assert.equal(actual?.length, heights.length, 'Source row minima must retain row order');
        heights.forEach((height, row) => {
          if (height == null) assert.equal(actual[row], height);
          else assert.ok(actual[row] != null && Math.abs(actual[row] - height) < 0.0001,
            `Table ${index} row ${row}: source minimum ${height}pt must survive parsing`);
        });
      });
    }
    const result = await render(doc);
    if (process.env.WORD_TABLE_EXPECT_PAGES) {
      assert.equal(result.pages, Number(process.env.WORD_TABLE_EXPECT_PAGES), 'Source page count must be preserved');
    }
    if (process.env.WORD_TABLE_EXPECT_PAGE_CONTENT) {
      const pageTexts = await page.locator('#host .page-body').allTextContents();
      const expected = JSON.parse(process.env.WORD_TABLE_EXPECT_PAGE_CONTENT);
      for (const [number, markers] of Object.entries(expected)) {
        for (const marker of markers) {
          assert.ok(pageTexts[Number(number) - 1]?.includes(marker), `Page ${number} must contain ${marker}`);
          assert.equal(pageTexts.filter(text => text.includes(marker)).length, 1, `${marker} must occur on only its expected page`);
        }
      }
    }
    const layout = await page.evaluate(() => {
      const shadow = document.querySelector('#host').shadowRoot;
      const tables = Array.from(shadow.querySelectorAll('table'), table => {
        const rect = table.getBoundingClientRect();
        const body = table.closest('.page-body');
        const bounds = body?.getBoundingClientRect();
        const wrapper = table.closest('.tbl-wrap');
        return { rows: table.rows.length, text: table.textContent.slice(0, 100),
          page: Array.from(shadow.querySelectorAll('.page-body')).indexOf(body) + 1,
          width: rect.width, height: rect.height, top: rect.top, bottom: rect.bottom,
          left: rect.left, right: rect.right, bodyLeft: bounds?.left, bodyRight: bounds?.right,
          bodyTop: bounds?.top, bodyBottom: bounds?.bottom,
          wrapperWidth: wrapper?.clientWidth, scrollWidth: wrapper?.scrollWidth,
          layout: getComputedStyle(table).tableLayout,
          columns: Array.from(table.querySelectorAll('col'), col => col.getBoundingClientRect().width) };
      });
      const cells = Array.from(shadow.querySelectorAll('.page-body td,.page-body th'), cell => {
        const rect = cell.getBoundingClientRect();
        const bounds = cell.closest('.page-body').getBoundingClientRect();
        const style = getComputedStyle(cell);
        return { text: cell.textContent, visible: rect.width > 0 && rect.height > 0 && style.visibility === 'visible'
          && rect.top >= bounds.top - 1 && rect.bottom <= bounds.bottom + 1 };
      });
      return { tables, cells };
    });
    await writeFile(resolve(output, 'external-document-layout.json'), JSON.stringify({ stats: doc.stats, pages: result.pages, renderedCells: result.cells.length,
      sourceRowMinHeightsPt: sourceTables.map(table => table.rowMinHeightsPt), ...layout }, null, 2));
    const sourceTexts = sourceTables.flatMap(table => table.grid.flat().filter(slot => slot.cell).map(slot => slot.cell.blocks.map(block => block.content?.map(run => run.text || '').join('') || '').join('')));
    const logical = await page.evaluate(collectWordFlowContent);
    const renderedTexts = new Map();
    for (const text of logical.cells) renderedTexts.set(text, (renderedTexts.get(text) || 0) + 1);
    const missing = sourceTexts.filter(text => {
      const remaining = renderedTexts.get(text) || 0;
      if (remaining) renderedTexts.set(text, remaining - 1);
      return !remaining;
    });
    const first = page.locator('#host table').first();
    if (await first.count()) await first.screenshot({ path: resolve(output, 'external-document-first-table.png') });
    await page.locator('#host .page').first().screenshot({ path: resolve(output, 'external-document-first-page.png') });
    await page.locator('#host table').last().screenshot({ path: resolve(output, 'external-document-last-table.png') });
    assert.ok(layout.tables.length >= sourceTables.length, `Rendered ${layout.tables.length}/${sourceTables.length} tables`);
    assert.equal(logical.cells.length, sourceTexts.length, 'Page fragments must not add or lose source cells');
    assert.equal(missing.length, 0, `Missing ${missing.length} cell texts: ${missing.slice(0, 3).join(', ')}`);
    assert.ok(layout.tables.every(table => table.width > 0 && table.height > 0), 'Tables must have visible bounds');
    assert.ok(layout.cells.every(cell => cell.visible), 'Table cells must stay inside the visible page body');
    if (process.env.WORD_TABLE_EXPECT_NO_STROKES === '1') {
      const strokes = result.cells.flatMap(cell => Object.values(cell.edges))
        .filter(edge => edge.style !== 'none' && parseFloat(edge.width) > 0);
      assert.equal(strokes.length, 0, 'This reference document must show only editing guides, without visible source strokes');
    }
    if (process.env.WORD_TABLE_EXPECT_SOLID_GRID === '1') {
      for (const cell of result.cells) {
        for (const [side, border] of Object.entries(cell.edges)) {
          assert.equal(border.style, 'solid', `${cell.text.slice(0, 30)} ${side}: source solid border must survive parsing`);
          assert.ok(parseFloat(border.width) > 0, `${side}: source border must have visible width`);
        }
      }
      assert.ok(result.cells.some(cell => parseFloat(cell.edges.left.width) > parseFloat(cell.edges.bottom.width)),
        'The thicker outside border must remain distinct from the inside grid');
    }
    if (process.env.WORD_TABLE_EXPECT_FIT === '1') {
      assert.ok(layout.tables.every(table => table.left >= table.bodyLeft - 1 && table.right <= table.bodyRight + 1),
        'Source table must fit the page body horizontally');
      assert.ok(layout.tables.every(table => table.scrollWidth <= table.wrapperWidth + 1),
        'Source table must not introduce a horizontal scrollbar');
      for (const rendered of layout.tables) {
        assert.equal(rendered.layout, 'fixed', 'This source form uses fixed column widths');
        for (const [index, width] of rendered.columns.entries()) {
          assert.ok(Math.abs(width / rendered.width - sourceTables[0].colWidthsPct[index]) < 0.005,
            `Column ${index}: preserve the source grid ratio in every page fragment`);
        }
      }
    }
    if (process.env.WORD_TABLE_EXPECT_SINGLE_LINE) {
      const texts = JSON.parse(process.env.WORD_TABLE_EXPECT_SINGLE_LINE);
      const lines = await page.locator('#host td p').evaluateAll((paragraphs, texts) => texts.map(text => {
        const paragraph = paragraphs.find(p => p.textContent.includes(text));
        if (!paragraph) return { text, missing: true };
        const range = document.createRange(); range.selectNodeContents(paragraph);
        const tops = [...new Set(Array.from(range.getClientRects(), rect => Math.round(rect.top)))];
        return { text: paragraph.textContent, lines: tops.length };
      }), texts);
      await writeFile(resolve(output, 'external-document-cell-lines.json'), JSON.stringify(lines, null, 2));
      for (const cell of lines) assert.equal(cell.lines, 1, `${cell.text}: text fits this source cell and must stay on one line`);
    }
    if (process.env.WORD_TABLE_EXPECT_SIGNATURE_ALIGNMENT) {
      const labels = JSON.parse(process.env.WORD_TABLE_EXPECT_SIGNATURE_ALIGNMENT);
      assert.deepEqual(doc.typography, { balanceSingleByteDoubleByteWidth: true, underlineTrailingSpaces: true, compressPunctuation: true });
      const paragraphs = sourceTables.flatMap(table => table.grid.flat()).flatMap(slot => slot.cell?.blocks ?? []);
      for (const label of labels) {
        const paragraph = paragraphs.find(block => block.content?.some(run => run.text?.includes(label)));
        assert.ok(paragraph, `Missing source signature: ${label}`);
        assert.ok(paragraph.content.every(run => run.font === '宋体' && run.sizeHalfPt === 21),
          'The DOC font table and inherited 10.5pt style must survive the WASM boundary');
      }
      const measure = () => page.locator('#host td p').evaluateAll((paragraphs, labels) => {
        const firstGlyph = paragraph => {
          const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
          while (walker.nextNode()) {
            const node = walker.currentNode, offset = node.textContent.search(/\S/);
            if (offset < 0) continue;
            const range = document.createRange(); range.setStart(node, offset); range.setEnd(node, offset + 1);
            return range.getBoundingClientRect().x;
          }
        };
        return labels.map(label => {
          const index = paragraphs.findIndex(p => p.textContent.includes(label));
          const signature = paragraphs[index], date = paragraphs[index + 1];
          return { label, signatureX: firstGlyph(signature), dateX: firstGlyph(date),
            difference: Math.abs(firstGlyph(signature) - firstGlyph(date)),
            text: signature.textContent + date.textContent };
        });
      }, labels);
      const after = await measure();
      await render({ ...doc, typography: undefined });
      const withoutControls = await measure();
      await render(doc);
      const repeated = await measure();
      assert.deepEqual(repeated, after, 'Re-rendering must not accumulate spacing adjustments');
      assert.ok(withoutControls.some(pair => pair.difference > 50), 'The regression must expose the missing source controls');
      for (const [index, pair] of after.entries()) {
        assert.ok(pair.difference < 5, `${pair.label}: alignment difference ${pair.difference}px`);
        assert.equal(pair.text, withoutControls[index].text, 'Source text and all spaces must be retained');
      }
      await writeFile(resolve(output, 'external-document-signatures.json'), JSON.stringify({ after, withoutControls }, null, 2));
    }
    return { pages: result.pages, sourceTables: sourceTables.length, renderedTables: layout.tables.length, sourceCells: sourceTexts.length, cells: result.cells.length };
  });
  await check('source-typography-controls-are-opt-in-and-preserve-text', async () => {
    const paragraph = font => `<w:p><w:pPr><w:spacing w:before="0" w:after="0"/><w:ind w:firstLine="400"/></w:pPr>
      <w:r><w:rPr><w:rFonts w:ascii="${font}" w:eastAsia="${font}"/><w:sz w:val="21"/></w:rPr><w:t xml:space="preserve">中  文（甲）：</w:t></w:r>
      <w:r><w:rPr><w:rFonts w:ascii="${font}" w:eastAsia="${font}"/><w:sz w:val="21"/><w:u w:val="single"/></w:rPr><w:t xml:space="preserve">      </w:t></w:r></w:p>`;
    const body = paragraph('宋体') + paragraph('Arial');
    const controls = '<w:characterSpacingControl w:val="compressPunctuation"/><w:compat><w:balanceSingleByteDoubleByteWidth/><w:ulTrailSpace/></w:compat>';
    const on = await source('typography-on', body, controls);
    const off = await source('typography-off', body, controls.replace('<w:balanceSingleByteDoubleByteWidth/>', '<w:balanceSingleByteDoubleByteWidth w:val="false"/>').replace('<w:ulTrailSpace/>', '<w:ulTrailSpace w:val="0"/>').replace('compressPunctuation', 'doNotCompress'));
    assert.deepEqual(on.typography, { balanceSingleByteDoubleByteWidth: true, underlineTrailingSpaces: true, compressPunctuation: true });
    assert.deepEqual(off.typography, { balanceSingleByteDoubleByteWidth: false, underlineTrailingSpaces: false, compressPunctuation: false });
    const measure = () => page.locator('#host .page-body p').evaluateAll(paragraphs => paragraphs.map(p => {
      const run = p.firstElementChild, text = run.firstChild;
      const space = document.createRange(); space.setStart(text, 1); space.setEnd(text, 2);
      return { text: p.textContent, space: space.getBoundingClientRect().width,
        size: parseFloat(getComputedStyle(run).fontSize),
        underline: p.lastElementChild.classList.contains('word-underlined-spaces'),
        underlineWidth: p.lastElementChild.getBoundingClientRect().width,
        compressed: run.dataset.wordPunctuation === 'true' };
    }));
    await render(on); const enabled = await measure();
    await render(off); const disabled = await measure();
    assert.ok(Math.abs(enabled[0].space - enabled[0].size / 2) < 0.05, 'Balanced CJK spaces must be half an em');
    assert.equal(enabled[0].compressed, true); assert.equal(disabled[0].compressed, false);
    assert.equal(enabled[0].underline, true); assert.equal(disabled[0].underline, false);
    assert.ok(Math.abs(enabled[0].underlineWidth - 6 * enabled[0].size / 2) < 0.1,
      'A space underline must not inherit and repeat the paragraph first-line indent');
    assert.equal(enabled[1].space, disabled[1].space, 'Proportional Latin font metrics must remain unchanged');
    assert.equal(enabled[1].compressed, false);
    assert.deepEqual(enabled.map(p => p.text), disabled.map(p => p.text));
  });
  await check('source-trailing-underlines-stay-on-the-signature-line', async () => {
    const run = (value, underline = false) => `<w:r><w:rPr><w:rFonts w:ascii="宋体" w:eastAsia="宋体"/><w:sz w:val="24"/>${underline ? '<w:u w:val="single"/>' : ''}</w:rPr><w:t xml:space="preserve">${value}</w:t></w:r>`;
    const paragraph = (align, sizes) => `<w:p><w:pPr><w:jc w:val="${align}"/><w:ind w:firstLine="3200"/><w:spacing w:before="0" w:after="0" w:line="560" w:lineRule="exact"/></w:pPr>${run('签字：')}${sizes.map(size => run(' '.repeat(size), true)).join('')}${run(' ')}</w:p>`;
    const body = `<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid><w:gridCol w:w="5600"/></w:tblGrid><w:tr><w:tc>${paragraph('left', [25])}${paragraph('both', [8, 5, 12])}${paragraph('center', [8, 5, 12])}</w:tc></w:tr></w:tbl>`;
    const doc = await source('trailing-underlines', body, '<w:compat><w:balanceSingleByteDoubleByteWidth/><w:ulTrailSpace/></w:compat>');
    await writeFile(resolve(output, 'trailing-underlines-dto.json'), JSON.stringify(doc, null, 2));
    const expected = doc.blocks[0].table.grid[0][0].cell.blocks.map(p => p.content.map(run => run.text).join(''));
    const measure = (zoom = 1) => page.locator('#host td p').evaluateAll((paragraphs, zoom) => paragraphs.map(p => {
      const box = p.getBoundingClientRect(), wrapper = p.closest('.tbl-wrap');
      const label = p.firstElementChild.getBoundingClientRect();
      return { text: p.textContent, height: box.height / zoom, lineHeight: parseFloat(getComputedStyle(p).lineHeight),
        scroll: wrapper.scrollWidth, width: wrapper.clientWidth,
        underlines: [...p.querySelectorAll('.word-underlined-spaces')].map(run => {
          const rect = run.getBoundingClientRect(), style = getComputedStyle(run);
          const visibleWidth = parseFloat(style.backgroundSize);
          return { topDifference: Math.abs(rect.top - label.top) / zoom, visibleWidth,
            visibleRight: rect.left + visibleWidth * zoom, paragraphRight: box.right };
        }) };
    }), zoom);
    const measurements = [];
    for (const zoom of [1, 0.7, 1.5, 0.25, 2]) {
      await render(doc, zoom); const first = await measure(zoom);
      measurements.push({ zoom, paragraphs: first });
      await writeFile(resolve(output, 'trailing-underlines.json'), JSON.stringify(measurements, null, 2));
      await page.screenshot({ path: resolve(output, `trailing-underlines-${zoom}.png`), fullPage: true });
      assert.equal(first.length, expected.length, 'Hanging spaces must not split a paragraph across pages');
      for (const [index, p] of first.entries()) {
        assert.equal(p.text, expected[index], 'Keep every source space across split runs');
        assert.ok(Math.abs(p.height - p.lineHeight) < 1, 'Trailing underlined spaces must not create an extra line');
        assert.ok(p.scroll <= p.width + 1, 'Hanging decoration must not create a scrollbar');
        assert.ok(p.underlines.every(run => run.topDifference < 1), 'Every underline must follow the label baseline');
        assert.ok(p.underlines.some(run => run.visibleWidth > 0), 'At least the portion inside the source text area remains visible');
        assert.ok(p.underlines.every(run => run.visibleWidth === 0 || run.visibleRight <= run.paragraphRight + 1),
          'Only the underline decoration is clipped at the text-area edge');
      }
      await render(doc, zoom); assert.deepEqual(await measure(zoom), first, 'Repeated layout must not accumulate adjustments');
    }
    return { measurements };
  });
  await check('underlined-blanks-preserve-right-alignment-interior-space-and-hard-breaks', async () => {
    const run = (text, underline = false) => `<w:r><w:rPr><w:rFonts w:ascii="宋体" w:eastAsia="宋体"/><w:sz w:val="24"/>${underline ? '<w:u w:val="single"/>' : ''}</w:rPr><w:t xml:space="preserve">${text}</w:t></w:r>`;
    const paragraph = (content, props = '') => `<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="560" w:lineRule="exact"/>${props}</w:pPr>${content}</w:p>`;
    const body = paragraph(run('右对齐：') + run(' '.repeat(12), true), '<w:jc w:val="right"/>')
      + paragraph(run('行内：') + run(' '.repeat(5), true) + run('正文'))
      + paragraph(run('换行前：') + run(' '.repeat(5), true) + '<w:r><w:br/></w:r>' + run('下一行'))
      + paragraph(run('制表位：') + '<w:r><w:tab/></w:r>' + run(' '.repeat(25), true), '<w:tabs><w:tab w:val="left" w:pos="2800"/></w:tabs>');
    const doc = await source('underline-boundaries', body, '<w:compat><w:balanceSingleByteDoubleByteWidth/><w:ulTrailSpace/></w:compat>');
    await render(doc);
    const paragraphs = await page.locator('#host .page-body p').evaluateAll(paragraphs => paragraphs.map(p => {
      const underline = p.querySelector('.word-underlined-spaces'), rect = underline.getBoundingClientRect();
      const style = getComputedStyle(underline), range = document.createRange();
      range.selectNodeContents(p.firstElementChild); const label = range.getBoundingClientRect();
      range.selectNodeContents(underline); const text = range.getBoundingClientRect();
      const box = p.getBoundingClientRect(), tab = p.querySelector('.word-tab');
      return { text: p.textContent, height: box.height, lineHeight: parseFloat(getComputedStyle(p).lineHeight),
        width: rect.width, right: rect.right, paragraphRight: box.right, top: text.top,
        labelTop: label.top, display: style.display, tabRight: tab?.getBoundingClientRect().right,
        left: rect.left, decorationWidth: parseFloat(style.backgroundSize) };
    }));
    assert.equal(paragraphs.length, 4);
    const [right, interior, hardBreak, tab] = paragraphs;
    assert.ok(Math.abs(right.right - right.paragraphRight) < 1, 'Right-aligned blanks retain their alignment width');
    assert.ok(Math.abs(right.width - 96) < 1, 'Right-aligned source spaces retain their full width');
    for (const p of [right, interior, hardBreak]) {
      assert.equal(p.display, 'inline-block', 'Only paragraph-final blanks in compatible alignments may hang');
      assert.ok(Math.abs(p.top - p.labelTop) < 1, 'Interior and pre-break blanks stay on their source line');
    }
    assert.ok(Math.abs(hardBreak.height - hardBreak.lineHeight * 2) < 1, 'A source hard break must retain both lines');
    assert.ok(Math.abs(tab.height - tab.lineHeight) < 1, 'A tab before trailing blanks must not add a line');
    assert.ok(Math.abs(tab.left - tab.tabRight) < 1, 'The underline must start at the resolved source tab');
    assert.ok(tab.decorationWidth > 0 && tab.left + tab.decorationWidth <= tab.paragraphRight + 1,
      'Measure the visible underline after resolving the tab position');
    return { paragraphs };
  });
  await check('per-edge-direct-border-styles', async () => {
    const doc = await source('direct-borders', table([row([
      cell('EDGES', '<w:tcBorders><w:top w:val="dotted" w:sz="8" w:color="112233"/><w:right w:val="dashed" w:sz="16" w:color="445566"/><w:bottom w:val="double" w:sz="24" w:color="778899"/><w:left w:val="single" w:sz="4" w:color="AABBCC"/></w:tcBorders>'), cell('OTHER'),
    ])], borders()));
    const dto = dtoCell(doc, 'EDGES');
    assert.deepEqual(Object.fromEntries(Object.entries(dto.borders || {}).map(([side, border]) => [side, border.style])), { top: 'dotted', right: 'dashed', bottom: 'double', left: 'solid' });
    assert.equal(dto.borders.top.widthPt, 1); assert.equal(dto.borders.right.widthPt, 2); assert.equal(dto.borders.bottom.widthPt, 3); assert.equal(dto.borders.left.widthPt, 0.5);
    const result = await render(doc), cellStyle = find(result, 'EDGES');
    for (const [side, style] of Object.entries({ top: 'dotted', right: 'dashed', bottom: 'double', left: 'solid' })) assert.equal(cellStyle.edges[side].style, style);
    assert.equal(cellStyle.edges.top.color, 'rgb(17, 34, 51)'); assert.equal(cellStyle.edges.bottom.color, 'rgb(119, 136, 153)');
    assert.ok(parseFloat(cellStyle.edges.bottom.width) > parseFloat(cellStyle.edges.left.width));
    await page.screenshot({ path: resolve(output, 'direct-borders.png'), fullPage: true });
  });
  await check('source-minimum-row-height-preserves-form-writing-space', async () => {
    const doc = await source('minimum-row-height', table([
      row([cell('FORM FIELD'), cell('WRITE HERE')], '<w:trHeight w:val="1200" w:hRule="atLeast"/>'),
      row([cell('CONTENT MAY GROW'), cell('LINE ONE')], '<w:trHeight w:val="160" w:hRule="atLeast"/>'),
    ], borders()));
    const form = doc.blocks.find(block => block.table).table;
    assert.deepEqual(form.rowMinHeightsPt, [60, 8], 'Parse source row heights before rendering');
    await render(doc);
    const rows = await page.locator('#host tr').evaluateAll(rows => rows.map(row => ({
      text: row.textContent, height: row.getBoundingClientRect().height,
    })));
    assert.ok(rows.find(row => row.text.includes('FORM FIELD')).height >= 80 - 1,
      'The source minimum must preserve blank writing space');
    assert.ok(rows.find(row => row.text.includes('CONTENT MAY GROW')).height > 8 * 4 / 3,
      'A minimum height must allow text to grow without clipping');
    await page.screenshot({ path: resolve(output, 'source-minimum-row-height.png'), fullPage: true });
    return { sourceMinimaPt: form.rowMinHeightsPt, rows };
  });
  await check('oversized-cell-indent-keeps-text-inside-fixed-columns', async () => {
    const withIndent = (text, twips) => cell(text).replace('<w:pPr>',
      `<w:pPr><w:ind w:${twips < 0 ? 'hanging' : 'firstLine'}="${Math.abs(twips)}"/>`);
    const doc = await source('cell-indents', table([
      row([withIndent('培训内容', 4095), withIndent('NORMAL INDENT', 200)]),
      row([withIndent('HANGING INDENT', -200), withIndent('乌鲁木齐、哈密', 4095)]),
    ], borders()));
    await render(doc);
    const layout = await page.locator('#host table').evaluate(table => {
      const wrapper = table.closest('.tbl-wrap');
      return {
        width: wrapper.clientWidth, scrollWidth: wrapper.scrollWidth,
        paragraphs: Array.from(table.querySelectorAll('p'), paragraph => {
          const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
          const text = walker.nextNode();
          const range = document.createRange(); range.setStart(text, 0); range.setEnd(text, 1);
          const glyph = range.getBoundingClientRect(), cell = paragraph.closest('td').getBoundingClientRect();
          return { text: paragraph.textContent, indent: getComputedStyle(paragraph).textIndent,
            firstOffset: glyph.left - paragraph.getBoundingClientRect().left,
            right: glyph.right, cellRight: cell.right };
        }),
      };
    });
    assert.ok(layout.scrollWidth <= layout.width + 1, 'An oversized first-line indent must not create horizontal overflow');
    for (const paragraph of layout.paragraphs) assert.ok(paragraph.right <= paragraph.cellRight + 1,
      `${paragraph.text}: first glyph must remain within the fixed cell`);
    assert.ok(Math.abs(layout.paragraphs.find(p => p.text === 'NORMAL INDENT').firstOffset - 200 / 15) < 0.1,
      'An ordinary first-line indent must retain its source distance');
    assert.ok(parseFloat(layout.paragraphs.find(p => p.text === 'HANGING INDENT').indent) < 0,
      'Hanging indents must remain negative');
  });
  await check('oversized-cell-indent-does-not-split-fitting-values', async () => {
    const indented = text => cell(text).replace('<w:pPr>', '<w:pPr><w:ind w:firstLine="4095"/>');
    const doc = await source('fitting-cell-values', table([
      row([indented('2026.10.13-2026.10.15'), indented('乌乌鲁木齐、哈密')]),
      row([indented('长文本仍需按照单元格宽度正常换行，不能通过禁止换行来遮住或者溢出内容。'),
        indented('第一行').replace('</w:t>', '</w:t><w:br/><w:t>第二行</w:t>')]),
    ], borders()));
    await render(doc);
    const paragraphs = await page.locator('#host td p').evaluateAll(paragraphs => paragraphs.map(paragraph => {
      const range = document.createRange(); range.selectNodeContents(paragraph);
      const tops = [...new Set(Array.from(range.getClientRects(), rect => Math.round(rect.top)))];
      const wrapper = paragraph.closest('.tbl-wrap');
      return { text: paragraph.textContent, lines: tops.length, width: wrapper.clientWidth, scroll: wrapper.scrollWidth };
    }));
    for (const value of ['2026.10.13-2026.10.15', '乌乌鲁木齐、哈密']) {
      assert.equal(paragraphs.find(p => p.text === value).lines, 1, `${value}: preserve one-line layout when the whole value fits`);
    }
    assert.ok(paragraphs.find(p => p.text.startsWith('长文本')).lines > 1, 'Long text must still wrap naturally');
    assert.ok(paragraphs.find(p => p.text.startsWith('第一行')).lines >= 2, 'Explicit source line breaks must remain');
    assert.ok(paragraphs.every(p => p.scroll <= p.width + 1), 'Do not reintroduce table overflow');
  });
  await check('split-row-retains-remaining-source-writing-space', async () => {
    const lead = Array.from({ length: 10 }, (_, i) => p(`BEFORE ${i}`)).join('');
    const content = Array.from({ length: 8 }, (_, i) => p(`FORM LINE ${i}`)).join('');
    const doc = await source('split-minimum-height', lead + table([
      row([`<w:tc>${content}</w:tc>`, cell('NOTES')]),
    ], borders()));
    doc.blocks.find(block => block.table).table.rowMinHeightsPt = [140];
    await render(doc);
    const heights = await page.locator('#host tr').evaluateAll(rows => rows.map(row => row.getBoundingClientRect().height));
    assert.ok(heights.length >= 2, 'Fixture must exercise a row split across pages');
    assert.ok(heights.reduce((sum, height) => sum + height, 0) >= 140 * 4 / 3 - 1,
      'Row fragments together must retain the source minimum writing space');
    const logical = await page.evaluate(collectWordFlowContent);
    assert.equal((logical.cells.join('').match(/FORM LINE/g) || []).length, 8,
      'Splitting must preserve each source line exactly once');
  });
  await check('unsized-borders-do-not-invent-visible-strokes', async () => {
    const doc = await source('unsized-borders',
      table([row([cell('UNSIZED STYLE HEADER'), cell('UNSIZED STYLE HEADER B')]), row([cell('BODY'), cell('BODY B')])],
        '<w:tblStyle w:val="UnsizedHeader"/><w:tblLook w:firstRow="1"/>')
      + table([row([
        cell('UNSIZED DIRECT', '<w:tcBorders><w:bottom w:val="single"/></w:tcBorders>'),
        cell('SIZED DIRECT', '<w:tcBorders><w:bottom w:val="single" w:sz="4"/></w:tcBorders>'),
      ])])
      + table([row([
        cell('UNSIZED OVERRIDE', '<w:tcBorders><w:bottom w:val="single"/></w:tcBorders>'),
        cell('INHERITED STROKE'),
      ])], '<w:tblStyle w:val="Base"/>'));
    const result = await render(doc);
    await page.screenshot({ path: resolve(output, 'unsized-borders.png'), fullPage: true });
    for (const text of ['UNSIZED STYLE HEADER', 'UNSIZED STYLE HEADER B', 'UNSIZED DIRECT', 'UNSIZED OVERRIDE']) {
      assert.equal(dtoCell(doc, text).borders.bottom.style, 'none', `${text}: absent source width must stay invisible`);
      assert.equal(find(result, text).edges.bottom.style, 'none', `${text}: renderer must retain the missing stroke`);
    }
    assert.equal(find(result, 'SIZED DIRECT').edges.bottom.style, 'solid');
    assert.equal(find(result, 'INHERITED STROKE').edges.bottom.style, 'solid');
  });
  await check('nil-and-none-override-inherited-grid', async () => {
    const doc = await source('no-border', table([row([cell('CLEAR', '<w:tcBorders><w:top w:val="nil"/><w:left w:val="none"/><w:bottom w:val="nil"/><w:right w:val="none"/></w:tcBorders>'), cell('GRID')])], '<w:tblStyle w:val="Base"/>'));
    const clear = dtoCell(doc, 'CLEAR');
    for (const edge of Object.values(clear.borders || {})) assert.equal(edge.style, 'none');
    assert.equal(Object.keys(clear.borders || {}).length, 4);
    const result = await render(doc);
    for (const edge of Object.values(find(result, 'CLEAR').edges)) assert.equal(edge.style, 'none');
    assert.equal(find(result, 'GRID').edges.top.style, 'solid');
  });
  await check('row-table-exceptions-retain-dashed-and-dotted-lines', async () => {
    const exceptions = '<w:tblBorders><w:top w:val="dashed" w:sz="16" w:color="112233"/><w:bottom w:val="dotted" w:sz="12" w:color="445566"/><w:insideV w:val="dotted" w:sz="8" w:color="778899"/></w:tblBorders>';
    const doc = await source('row-exceptions', table([row([
      cell('DIRECT OVERRIDE', '<w:tcBorders><w:top w:val="double" w:sz="24" w:color="AA2233"/></w:tcBorders>'),
      cell('ROW DASHES'),
    ], '', exceptions)], borders()));
    const dto = dtoCell(doc, 'ROW DASHES');
    assert.equal(dto.borders.top.style, 'dashed'); assert.equal(dto.borders.top.widthPt, 2);
    assert.equal(dto.borders.bottom.style, 'dotted'); assert.equal(dto.borders.bottom.widthPt, 1.5);
    const result = await render(doc), dashed = find(result, 'ROW DASHES');
    assert.equal(dashed.edges.top.style, 'dashed'); assert.equal(dashed.edges.top.color, 'rgb(17, 34, 51)');
    assert.equal(dashed.edges.bottom.style, 'dotted'); assert.equal(dashed.edges.bottom.color, 'rgb(68, 85, 102)');
    assert.equal(dashed.edges.left.style, 'dotted');
    assert.equal(find(result, 'DIRECT OVERRIDE').edges.top.style, 'double');
    await page.screenshot({ path: resolve(output, 'row-table-exceptions.png'), fullPage: true });
  });
  await check('merged-cell-retains-final-row-table-exception', async () => {
    const doc = await source('merged-row-exceptions', table([
      row([cell('MERGED ROW BORDER', '<w:vMerge w:val="restart"/>'), cell('TOP')], '', '<w:tblBorders><w:top w:val="dashed" w:sz="8"/></w:tblBorders>'),
      row([cell('', '<w:vMerge/>'), cell('BOTTOM')], '', '<w:tblBorders><w:bottom w:val="dotted" w:sz="16" w:color="334455"/></w:tblBorders>'),
    ], borders()));
    const result = await render(doc), merged = find(result, 'MERGED ROW BORDER');
    assert.equal(merged.rowSpan, 2); assert.equal(merged.edges.top.style, 'dashed');
    assert.equal(merged.edges.bottom.style, 'dotted'); assert.equal(merged.edges.bottom.color, 'rgb(51, 68, 85)');
  });
  await check('interior-row-table-exceptions-keep-horizontal-edges', async () => {
    const doc = await source('interior-row-exceptions', table([
      row([cell('BEFORE'), cell('BEFORE B')]),
      row([cell('INTERIOR'), cell('INTERIOR B')], '', '<w:tblBorders><w:top w:val="dashed" w:sz="16"/><w:bottom w:val="dotted" w:sz="16"/></w:tblBorders>'),
      row([cell('AFTER'), cell('AFTER B')]),
    ], borders()));
    const dto = dtoCell(doc, 'INTERIOR');
    assert.equal(dto.borders.top.style, 'dashed'); assert.equal(dto.borders.bottom.style, 'dotted');
    const result = await render(doc), interior = find(result, 'INTERIOR');
    assert.equal(interior.edges.top.style, 'dashed'); assert.equal(interior.edges.bottom.style, 'dotted');
    assert.equal(find(result, 'BEFORE').edges.top.style, 'solid');
    assert.equal(find(result, 'AFTER').edges.bottom.style, 'solid');
  });
  await check('conditional-header-banding-and-inheritance', async () => {
    const props = '<w:tblStyle w:val="Derived"/><w:tblLook w:firstRow="1" w:firstColumn="0" w:noHBand="1" w:noVBand="1"/>';
    const doc = await source('conditional-header', table([
      row([cell('STYLED HEADER'), cell('DIRECT FILL', '<w:shd w:val="clear" w:fill="DDEEFF"/>')]),
      row([cell('STRIPE 1'), cell('STRIPE 1B')]), row([cell('STRIPE 2'), cell('STRIPE 2B')]),
    ], props));
    assert.equal(dtoCell(doc, 'STYLED HEADER').isHeader, false, 'Conditional firstRow styling must not require a repeating header flag');
    const result = await render(doc);
    assert.equal(find(result, 'STYLED HEADER').background, 'rgb(47, 84, 150)');
    assert.equal(find(result, 'DIRECT FILL').background, 'rgb(221, 238, 255)');
    assert.equal(find(result, 'STYLED HEADER').edges.top.style, 'solid');
    assert.equal(find(result, 'STYLED HEADER').edges.top.color, 'rgb(85, 85, 85)');
    assert.equal(find(result, 'STYLED HEADER').edges.right.style, 'dotted');
    assert.equal(find(result, 'DIRECT FILL').edges.left.style, 'dotted');
    assert.equal(find(result, 'STYLED HEADER').edges.left.style, 'solid');
    await page.screenshot({ path: resolve(output, 'conditional-header.png'), fullPage: true });
  });
  await check('banded-fill-without-header-row', async () => {
    const doc = await source('banded-fill', table([
      row([cell('BAND ONE'), cell('BAND ONE B')]), row([cell('BAND TWO'), cell('BAND TWO B')]), row([cell('BAND ONE AGAIN'), cell('BAND ONE AGAIN B')]),
    ], '<w:tblStyle w:val="Derived"/><w:tblLook w:firstRow="0" w:firstColumn="0" w:noHBand="0" w:noVBand="1"/>'));
    const result = await render(doc);
    for (const text of ['BAND ONE', 'BAND ONE AGAIN']) assert.equal(find(result, text).background, 'rgb(234, 241, 251)');
    assert.notEqual(find(result, 'BAND TWO').background, 'rgb(234, 241, 251)');
  });
  await check('theme-fills-tint-shade-and-direct-override', async () => {
    const doc = await source('theme-fill', table([
      row([cell('THEME HEADER'), cell('THEME SHADE', '<w:shd w:val="clear" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillShade="80"/>')]),
      row([cell('THEME DIRECT', '<w:shd w:val="clear" w:fill="FFFFFF" w:themeFill="accent1"/>'), cell('EXPLICIT CLEAR', '<w:shd w:val="nil"/>')]),
    ], '<w:tblStyle w:val="Theme"/><w:tblLook w:firstRow="1"/>'));
    const result = await render(doc);
    // OOXML changes HSL luminance; independent RGB channel blending is incorrect.
    // https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/8229a077-7fc8-4fba-96cc-c77b6a4fc768
    assert.equal(find(result, 'THEME HEADER').background, 'rgb(111, 159, 207)');
    assert.equal(find(result, 'THEME SHADE').background, 'rgb(16, 32, 48)');
    assert.equal(find(result, 'THEME DIRECT').background, 'rgb(32, 64, 96)');
    assert.equal(find(result, 'EXPLICIT CLEAR').background, 'rgba(0, 0, 0, 0)');
  });
  await check('theme-mapping-and-solid-foreground-fill', async () => {
    const doc = await source('mapped-theme-fill', table([row([
      cell('REMAPPED', '<w:shd w:val="clear" w:fill="FFFFFF" w:themeFill="accent1"/>'),
      cell('SOLID', '<w:shd w:val="solid" w:color="112233" w:fill="FFFFFF"/>'),
    ])]), '<w:clrSchemeMapping w:accent1="accent2"/>');
    const result = await render(doc);
    assert.equal(find(result, 'REMAPPED').background, 'rgb(160, 176, 192)');
    assert.equal(find(result, 'SOLID').background, 'rgb(17, 34, 51)');
  });
  await check('editing-gridlines-preserve-source-borders-and-do-not-print', async () => {
    const doc = await source('unstyled-table', table([row([cell('PLAIN HEADER'), cell('PLAIN HEADER B')], '<w:tblHeader/>'), row([cell('PLAIN BODY'), cell('PLAIN BODY B')])]));
    const result = await render(doc);
    assert.equal(find(result, 'PLAIN HEADER').tag, 'TH');
    for (const cellStyle of result.cells) {
      assert.equal(cellStyle.background, 'rgba(0, 0, 0, 0)');
      for (const edge of Object.values(cellStyle.edges)) assert.equal(edge.style, 'none');
    }
    const geometry = await page.locator('#host table').evaluate(table => {
      const rect = table.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    });
    const screenGuides = await page.locator('#host td').first().evaluate(cell => getComputedStyle(cell).backgroundImage);
    assert.ok(screenGuides.includes('repeating-linear-gradient'), 'Borderless cells need visible editing gridlines');
    await page.screenshot({ path: resolve(output, 'borderless-editing-gridlines.png'), fullPage: true });
    await page.emulateMedia({ media: 'print' });
    try {
      assert.equal(await page.locator('#host td').first().evaluate(cell => getComputedStyle(cell).backgroundImage), 'none', 'Editing gridlines must not print');
      const printed = await page.locator('#host table').evaluate(table => {
        const rect = table.getBoundingClientRect();
        return { width: rect.width, height: rect.height };
      });
      assert.deepEqual(printed, geometry, 'Editing gridlines must not affect table geometry');
    } finally { await page.emulateMedia({ media: 'screen' }); }
  });
  await check('merged-cell-style-survives-grid-normalization', async () => {
    const doc = await source('merged-cell', table([
      row([cell('HORIZONTAL', '<w:gridSpan w:val="2"/><w:shd w:val="clear" w:fill="AADDCC"/>'), cell('TOP RIGHT')]),
      row([cell('VERTICAL', '<w:vMerge w:val="restart"/><w:shd w:val="clear" w:fill="CCDDEE"/>'), cell('MIDDLE'), cell('RIGHT')]),
      row([cell('', '<w:vMerge/><w:tcBorders><w:bottom w:val="double" w:sz="24" w:color="CC2233"/></w:tcBorders>'), cell('BOTTOM'), cell('BOTTOM RIGHT')]),
    ], borders('dashed', 8, '225588'), [1600, 1600, 1600]));
    const result = await render(doc);
    assert.equal(find(result, 'HORIZONTAL').colSpan, 2); assert.equal(find(result, 'VERTICAL').rowSpan, 2);
    assert.equal(find(result, 'HORIZONTAL').background, 'rgb(170, 221, 204)');
    assert.equal(find(result, 'VERTICAL').background, 'rgb(204, 221, 238)');
    for (const edge of Object.values(find(result, 'HORIZONTAL').edges)) assert.equal(edge.style, 'dashed');
    for (const side of ['top', 'right', 'left']) assert.equal(find(result, 'VERTICAL').edges[side].style, 'dashed');
    assert.equal(find(result, 'VERTICAL').edges.bottom.style, 'double');
    assert.equal(find(result, 'VERTICAL').edges.bottom.color, 'rgb(204, 34, 51)');
  });
  await check('repeated-header-retains-source-fill-and-borders', async () => {
    const rows = [row([cell('REPEATED HEADER'), cell('REPEATED HEADER B')], '<w:tblHeader/>')];
    for (let index = 0; index < 35; index++) rows.push(row([cell(`ROW ${index}`), cell(`VALUE ${index}`)]));
    const doc = await source('repeated-header', table(rows, '<w:tblStyle w:val="Derived"/><w:tblLook w:firstRow="1" w:noHBand="1"/>'));
    const result = await render(doc), headers = result.cells.filter(cell => cell.text === 'REPEATED HEADER');
    assert.ok(result.pages > 2); assert.equal(headers.length, result.pages);
    for (const header of headers) { assert.equal(header.background, 'rgb(47, 84, 150)'); assert.equal(header.edges.top.style, 'solid'); assert.equal(header.edges.top.color, 'rgb(85, 85, 85)'); }
    return { pages: result.pages, repeatedHeaders: headers.length };
  });
  await check('real-styles-word-table-retains-source-grid-and-fill', async () => {
    const doc = engine.toDocument(await readFile(resolve(root, 'apps/word-view/public/samples/styles-word.docx')));
    const result = await render(doc);
    await page.locator('#host table').first().screenshot({ path: resolve(output, 'styles-word-table.png') });
    assert.equal(result.cells.filter(cell => cell.background === 'rgb(221, 235, 247)').length, 3);
    for (const cellStyle of result.cells) for (const edge of Object.values(cellStyle.edges)) {
      assert.equal(edge.style, 'solid'); assert.equal(edge.color, 'rgb(0, 0, 0)');
    }
    return { pages: result.pages, cells: result.cells.length };
  });
} finally {
  await writeFile(resolve(output, 'results.json'), JSON.stringify({ results, errors }, null, 2));
  await browser.close(); await new Promise(done => server.close(done));
}
const failures = results.filter(result => !result.passed);
console.log(`${results.length - failures.length}/${results.length} Word table regression checks passed.`);
if (failures.length || errors.length) process.exitCode = 1;
