import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output/playwright/word-tabs');
const harness = await realpath(await mkdtemp(resolve(tmpdir(), 'word-tabs-')));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const engine = await import(pathToFileURL(resolve(root, 'packages/viewer-word/src/wasm/word_view_rs.js')).href);
engine.initSync({ module: await readFile(resolve(root, 'packages/viewer-word/src/wasm/word_view_rs_bg.wasm')) });
await mkdir(output, { recursive: true });
const w = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
const run = text => `<w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="20"/></w:rPr>${text.split('\t').map((part, i) => `${i ? '<w:tab/>' : ''}<w:t xml:space="preserve">${part}</w:t>`).join('')}</w:r>`;
const para = (text, props = '') => `<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240"/>${props}</w:pPr>${run(text)}</w:p>`;
const tabs = (...values) => `<w:tabs>${values.map(([position, alignment, leader = 'none']) => `<w:tab w:pos="${position * 20}" w:val="${alignment}" w:leader="${leader}"/>`).join('')}</w:tabs>`;

async function source(name, body, { header = '', styles = '', defaultTab = 36 } = {}) {
  const directory = resolve(harness, name);
  await mkdir(resolve(directory, 'word/_rels'), { recursive: true });
  await mkdir(resolve(directory, '_rels'), { recursive: true });
  const parts = {
    '[Content_Types].xml': `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>`,
    '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
    'word/_rels/document.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="styles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="settings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="header" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>',
    'word/styles.xml': `<w:styles xmlns:w="${w}">${styles}</w:styles>`,
    'word/settings.xml': `<w:settings xmlns:w="${w}"><w:defaultTabStop w:val="${defaultTab * 20}"/></w:settings>`,
    'word/header1.xml': `<w:hdr xmlns:w="${w}">${header}</w:hdr>`,
    'word/document.xml': `<w:document xmlns:w="${w}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>${body}<w:sectPr><w:headerReference w:type="default" r:id="header"/><w:pgSz w:w="7200" w:h="5600"/><w:pgMar w:top="900" w:right="600" w:bottom="600" w:left="600" w:header="300"/></w:sectPr></w:body></w:document>`,
  };
  for (const [part, text] of Object.entries(parts)) await writeFile(resolve(directory, part), text);
  const archive = resolve(harness, `${name}.docx`);
  execFileSync('zip', ['-q', '-r', archive, '.'], { cwd: directory });
  return engine.toDocument(await readFile(archive));
}

await writeFile(resolve(harness, 'index.html'), '<!doctype html><meta charset="utf-8"><title>Word source tab regression</title><div id="host" style="width:700px"></div><script type="module" src="/main.js"></script>');
await writeFile(resolve(harness, 'main.js'), `
import {renderWordInto} from ${JSON.stringify(resolve(root, 'packages/viewer-word/src/core/render.ts'))};
const host=document.querySelector('#host');let rendered;
window.wordTabs={async render(doc,zoom=1){
  rendered?.dispose();host.style.zoom=String(zoom);await document.fonts.ready;
  const idle=window.requestIdleCallback,cancel=window.cancelIdleCallback,pending=new Map();let next=0;
  window.requestIdleCallback=fn=>{pending.set(++next,fn);return next;};window.cancelIdleCallback=id=>pending.delete(id);
  try{
    rendered=renderWordInto(host,doc);let batches=0;
    while(pending.size){if(++batches>1000)throw new Error('Pagination did not settle');const [id,fn]=pending.entries().next().value;pending.delete(id);fn();}
    const rect=node=>{const r=node.getBoundingClientRect();return {left:r.left/zoom,right:r.right/zoom,top:r.top/zoom,bottom:r.bottom/zoom,width:r.width/zoom,height:r.height/zoom};};
    const paragraphs=Array.from(host.shadowRoot.querySelectorAll('p,h1,h2,li'),node=>{
      const range=document.createRange();range.selectNodeContents(node);
      const tabs=Array.from(node.querySelectorAll('.word-tab'));
      return {text:node.textContent,header:!!node.closest('.page-header'),box:rect(node),marginRight:getComputedStyle(node).marginRight,lineHeight:parseFloat(getComputedStyle(node).lineHeight),
        lines:[...new Set(Array.from(range.getClientRects(),r=>Math.round(r.top/zoom)))],
        tabs:tabs.map((tab,index)=>{
          const field=document.createRange();field.setStartAfter(tab);
          if(tabs[index+1])field.setEndBefore(tabs[index+1]);else field.setEnd(node,node.childNodes.length);
          return {box:rect(tab),field:rect(field),leader:tab.dataset.leader,content:getComputedStyle(tab,'::after').content};
        }),
        runs:Array.from(node.querySelectorAll(':scope > span'),span=>({text:span.textContent,box:rect(span),font:getComputedStyle(span).fontFamily,size:getComputedStyle(span).fontSize,color:getComputedStyle(span).color}))};
    });
    return {paragraphs,pages:host.shadowRoot.querySelectorAll('.page').length,text:Array.from(host.shadowRoot.querySelectorAll('.page-body'),node=>node.textContent).join('')};
  }finally{window.requestIdleCallback=idle;window.cancelIdleCallback=cancel;}
}};
`);
await build({ root: harness, configFile: false, base: '/word-tabs/', logLevel: 'warn', build: { target: 'es2022' } });
const dist = resolve(harness, 'dist');
const server = createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  const path = resolve(dist, decodeURIComponent(url.pathname.slice('/word-tabs/'.length)) || 'index.html');
  if (!url.pathname.startsWith('/word-tabs/') || !path.startsWith(dist + sep)) { response.writeHead(403).end(); return; }
  try { response.writeHead(200, { 'Content-Type': { '.html': 'text/html', '.js': 'text/javascript' }[extname(path)] || 'application/octet-stream' }).end(await readFile(path)); }
  catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 850 }, deviceScaleFactor: 1 });
const results = [], errors = [];
page.on('pageerror', error => errors.push(error.message));
const render = (doc, zoom = 1) => page.evaluate(({ doc, zoom }) => window.wordTabs.render(doc, zoom), { doc, zoom });
const close = (actual, expected, label) => assert.ok(Math.abs(actual - expected) < 1.5, `${label}: ${actual} vs ${expected}`);
const check = async (name, fn) => {
  try { await fn(); results.push({ name, passed: true }); console.log(`PASS ${name}`); }
  catch (error) { results.push({ name, passed: false, error: error.stack }); console.error(`FAIL ${name}: ${error.message}`); }
};
try {
  await page.goto(`http://127.0.0.1:${server.address().port}/word-tabs/`);
  await page.waitForFunction(() => !!window.wordTabs);
  if (process.env.WORD_HEADER_DOCX) await check('external-document-header-does-not-wrap', async () => {
    const doc = engine.toDocument(await readFile(resolve(process.env.WORD_HEADER_DOCX)));
    // Retain the actual source setup and header; a short body keeps this focused
    // check practical for documents with hundreds of pages.
    doc.blocks = doc.blocks.filter(block => block.kind === 'paragraph').slice(0, 2);
    for (const zoom of [1, 0.7, 1.5, 0.25, 2]) {
      const result = await render(doc, zoom), headers = result.paragraphs.filter(p => p.header);
      await writeFile(resolve(output, `external-header-${zoom}.json`), JSON.stringify(headers, null, 2));
      await page.locator('#host .page-header').first().screenshot({ path: resolve(output, `external-header-${zoom}.png`) });
      if (zoom === 1) await page.locator('#host .page').first().screenshot({ path: resolve(output, 'external-first-page.png') });
      for (const header of headers) {
        assert.ok(header.box.height < 30, `Source header wrapped at zoom ${zoom}: ${header.box.height}px`);
        const text = header.runs.filter(run => run.text.trim()).at(-1);
        close(text.box.right, header.box.right, 'Source header right alignment');
      }
      const image = await page.locator('#host .page-header img').first().evaluate(image => ({
        position: getComputedStyle(image).position, width: image.width, height: image.height,
        offset: { left: parseFloat(image.style.left), top: parseFloat(image.style.top) },
      }));
      assert.equal(image.position, 'absolute', 'Floating header logo must stay outside the text flow');
      assert.equal(image.width, 96); assert.equal(image.height, 26);
      close(image.offset.left, 57785 / 9525, 'Source logo horizontal offset');
      close(image.offset.top, 181610 / 9525, 'Source logo vertical offset');
    }
  });
  await check('renderer-aligns-mixed-runs-without-changing-text', async () => {
    const doc = await source('renderer-tabs', para('Body'));
    doc.setup.header = [{ kind: 'paragraph', tabStops: [
      { positionPt: 150, alignment: 'center', leader: 'none' },
      { positionPt: 300, alignment: 'right', leader: 'dot' },
    ], content: ['Company', '\tReport', '\t2026'].map(text => ({ kind: 'text', text, font: 'Arial', sizeHalfPt: 20 })) }];
    const p = (await render(doc)).paragraphs.find(p => p.header);
    assert.equal(p.text, 'Company\tReport\t2026'); assert.equal(p.tabs.length, 2);
    close(p.tabs[1].field.right, p.box.right, 'Right edge');
    close((p.tabs[0].field.left + p.tabs[0].field.right) / 2, p.box.left + 200, 'Center');
    assert.ok(p.box.height < 26);
    assert.ok(p.tabs[1].content.includes('...'), 'Dot decoration should be present');
    await page.screenshot({ path: resolve(output, 'renderer-header-tabs.png'), fullPage: true });
  });
  await check('floating-header-logo-and-song-font-preserve-one-line', async () => {
    const doc = await source('floating-header', para('Body'));
    doc.setup.page.widthPt = 612;
    doc.setup.page.leftPt = doc.setup.page.rightPt = 72;
    doc.setup.header = [{ kind: 'paragraph', align: 'right', linePct: 1.5, content: [
      { kind: 'image', src: 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7', widthPx: 96, heightPx: 26,
        anchor: { horizontalFrom: 'column', verticalFrom: 'paragraph', xPt: 4.55, yPt: 14.3 } },
      { kind: 'text', text: ' '.repeat(85), font: '宋体', sizeHalfPt: 24 },
      { kind: 'text', text: '智能化运销系统建设项目', font: '宋体', sizeHalfPt: 24 },
    ] }];
    for (const zoom of [0.25, 0.7, 1, 1.5, 2]) {
      const result = await render(doc, zoom), header = result.paragraphs.find(p => p.header);
      assert.equal(header.text, ' '.repeat(85) + '智能化运销系统建设项目', 'Source spaces must remain intact');
      assert.ok(header.box.height <= header.lineHeight + 1, `Header must occupy one source line at zoom ${zoom}`);
      assert.ok(header.runs.at(-1).font.includes('Songti SC'));
      close(header.runs.at(-1).box.right, header.box.right, 'Header text right edge');
    }
  });
  await check('source-toc-cache-style-and-dot-leader', async () => {
    const body = `<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> TOC \\o "1-3" \\h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r></w:p>${para('Cached chapter\t12', tabs([280, 'right', 'dot']))}<w:p><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>${para('Actual heading', '<w:outlineLvl w:val="0"/>')}`;
    const doc = await source('cached-toc', body);
    assert.ok(doc.blocks.some(block => block.content?.some(run => run.text === 'Cached chapter\t12')), 'Source TOC cache was replaced');
    assert.equal(doc.blocks.find(block => block.tabStops?.length)?.tabStops[0].leader, 'dot');
    const result = await render(doc), paragraph = result.paragraphs.find(p => p.text === 'Cached chapter\t12');
    assert.ok(paragraph); assert.equal(paragraph.tabs.length, 1); assert.equal(paragraph.tabs[0].leader, 'dot');
    close(paragraph.tabs[0].field.right, paragraph.box.left + 280 * 4 / 3, 'TOC page number endpoint');
    assert.equal(result.text.split('Actual heading').length - 1, 1, 'Preview must not synthesize another heading inside TOC');
    await page.screenshot({ path: resolve(output, 'source-toc-leader.png'), fullPage: true });
  });
  await check('empty-toc-does-not-invent-a-directory', async () => {
    const doc = await source('empty-toc', `<w:p><w:fldSimple w:instr=' TOC \\o "1-3" '/></w:p>${para('Heading only', '<w:outlineLvl w:val="0"/>')}`);
    const result = await render(doc);
    assert.equal(result.text.split('Heading only').length - 1, 1);
  });
  await check('headings-without-toc-do-not-create-a-directory', async () => {
    const doc = await source('no-toc', para('Just the source heading', '<w:outlineLvl w:val="0"/>') + para('Body'));
    assert.equal((await render(doc)).text, 'Just the source headingBody');
  });
  await check('center-and-right-header-tabs-stay-on-one-line', async () => {
    const header = `<w:p><w:pPr><w:spacing w:before="0" w:after="0"/>${tabs([150, 'center'], [300, 'right'])}</w:pPr>${run('Company')}${run('\tReport')}${run('\t2026')}</w:p>`;
    const doc = await source('header-tabs', para('Body'), { header });
    for (const zoom of [1, 0.7, 1.5, 0.25, 2]) {
      const result = await render(doc, zoom), p = result.paragraphs.find(p => p.header);
      await writeFile(resolve(output, `header-${zoom}.json`), JSON.stringify(p, null, 2));
      await page.screenshot({ path: resolve(output, `source-header-${zoom}.png`), fullPage: true });
      assert.equal(p.text, 'Company\tReport\t2026'); assert.equal(p.tabs.length, 2);
      const content = p.runs.filter(run => run.text && !run.text.startsWith('\t'));
      assert.ok(p.box.height < 26, `Header wrapped at zoom ${zoom}`);
      close(p.runs.at(-1).box.right, p.box.right, 'Right header endpoint');
      const center = p.tabs[0].field;
      close((center.left + center.right) / 2, p.box.left + 150 * 4 / 3, 'Center header endpoint');
      assert.ok(content[0].font.includes('Arial')); assert.equal(content[0].size, '13.3333px');
    }
    await page.screenshot({ path: resolve(output, 'source-header-tabs.png'), fullPage: true });
  });
  await check('default-tab-interval-and-no-fabricated-leader', async () => {
    const doc = await source('default-tab', `<w:p>${run('A')}${run('\tB')}</w:p>`, { defaultTab: 48 });
    assert.equal(doc.defaultTabStopPt, 48);
    const p = (await render(doc)).paragraphs[0]; assert.equal(p.tabs.length, 1);
    close(p.tabs[0].box.right - p.box.left, 48 * 4 / 3, 'Default stop');
    assert.ok(!p.tabs[0].leader || p.tabs[0].leader === 'none');
  });
  await check('header-retains-source-exact-and-minimum-line-spacing', async () => {
    const header = ['exact', 'atLeast'].map((rule, index) => `<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="${(index ? 24 : 18) * 20}" w:lineRule="${rule}"/></w:pPr>${run(rule)}</w:p>`).join('');
    const doc = await source('header-spacing', para('Body'), { header });
    assert.deepEqual(doc.setup.header.map(block => block.lineSpacing), [
      { rule: 'exact', valuePt: 18 }, { rule: 'atLeast', valuePt: 24 },
    ]);
    const paragraphs = (await render(doc)).paragraphs.filter(p => p.header);
    close(paragraphs[0].box.height, 24, 'Exact header line height');
    close(paragraphs[1].box.height, 32, 'Minimum header line height');
  });
  await check('header-does-not-invent-a-smaller-gray-font', async () => {
    const plain = text => `<w:p><w:r><w:t>${text}</w:t></w:r></w:p>`;
    const doc = await source('inherited-header-font', plain('Body inherited'), { header: plain('Header inherited') });
    const result = await render(doc), body = result.paragraphs.find(p => !p.header), header = result.paragraphs.find(p => p.header);
    assert.equal(header.runs[0].size, body.runs[0].size, 'Unspecified header size should inherit document defaults');
    assert.equal(header.runs[0].color, body.runs[0].color, 'Unspecified header color should inherit document defaults');
  });
  await check('inherited-stop-clear-and-right-indent', async () => {
    const styles = `<w:style w:type="paragraph" w:styleId="Tabbed"><w:pPr>${tabs([100, 'left', 'dot'], [280, 'right'])}</w:pPr></w:style>`;
    const doc = await source('inherited-tabs', para('Label\t9', `<w:pStyle w:val="Tabbed"/>${tabs([100, 'clear'])}<w:ind w:right="-200"/>`), { styles });
    assert.equal(doc.blocks[0].indentRightPt, -10);
    const p = (await render(doc)).paragraphs[0]; assert.equal(p.tabs[0].leader, 'none');
    close(parseFloat(p.marginRight), -10 * 4 / 3, 'Negative right indent');
    assert.ok(p.tabs[0].box.width > 200);
  });
  await check('table-cell-tabs-retain-local-origin', async () => {
    const table = `<w:tbl><w:tblPr><w:tblW w:type="pct" w:w="5000"/><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid><w:gridCol w:w="6000"/></w:tblGrid><w:tr><w:tc>${para('Cell\t8', tabs([240, 'right', 'dot']))}</w:tc></w:tr></w:tbl>`;
    const doc = await source('cell-tabs', table);
    const p = (await render(doc)).paragraphs.find(p => p.text === 'Cell\t8');
    assert.equal(p.tabs.length, 1); assert.equal(p.tabs[0].leader, 'dot');
    assert.ok(p.tabs[0].box.width > 240);
  });
  await check('real-header-line-breaks-are-preserved', async () => {
    const doc = await source('multiline-header', para('Body'), { header: `<w:p>${run('Line one')}<w:r><w:br/></w:r>${run('Line two')}</w:p>` });
    const p = (await render(doc)).paragraphs.find(p => p.header);
    assert.ok(p.text.includes('\n')); assert.ok(p.box.height >= 26);
  });
} finally {
  await writeFile(resolve(output, 'results.json'), JSON.stringify({ results, errors }, null, 2));
  await browser.close(); await new Promise(done => server.close(done));
}
console.log(`${results.filter(result => result.passed).length}/${results.length} Word source tab regression checks passed.`);
if (results.some(result => !result.passed) || errors.length) process.exitCode = 1;
