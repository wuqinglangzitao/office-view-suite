import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { collectWordFlowContent } from './word-flow-content.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output/playwright/word-pagination');
const baseline = process.argv.includes('--baseline');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const harness = await realpath(await mkdtemp(resolve(tmpdir(), 'word-pagination-')));
await mkdir(output, { recursive: true });
let renderer = resolve(root, 'packages/viewer-word/src/core/render.ts');
if (baseline) {
  for (const file of ['render.ts', 'styles.ts']) {
    await writeFile(resolve(harness, file), execFileSync('git', ['show', `HEAD:packages/viewer-word/src/core/${file}`], { cwd: root }));
  }
  renderer = resolve(harness, 'render.ts');
}
await writeFile(resolve(harness, 'index.html'), '<!doctype html><meta charset="utf-8"><title>Word pagination regression</title><style>body{margin:0;background:#ddd}#host{width:700px}</style><div id="host"></div><script type="module" src="/main.js"></script>');
await writeFile(resolve(harness, 'main.js'), `
import {renderWordInto} from ${JSON.stringify(renderer)};
const host=document.querySelector('#host');let rendered;
window.wordPagination={
  async render(doc,scale=1){
    rendered?.dispose();host.style.zoom=String(scale);await document.fonts.ready;
    const idle=window.requestIdleCallback,cancel=window.cancelIdleCallback;
    const pending=new Map();let next=0;
    window.requestIdleCallback=fn=>{pending.set(++next,fn);return next;};
    window.cancelIdleCallback=id=>pending.delete(id);
    try{
      rendered=renderWordInto(host,doc);
      let batches=0;while(pending.size){if(++batches>1000)throw new Error('Pagination did not settle');const [id,fn]=pending.entries().next().value;pending.delete(id);fn();}
      const pages=Array.from(host.shadowRoot.querySelectorAll('.page-body'),body=>{
        const bounds=body.getBoundingClientRect();
        return {text:body.textContent,blocks:Array.from(body.children,node=>({text:node.textContent,tag:node.tagName})),
          overflow:Array.from(body.querySelectorAll('p,li,h1,h2')).filter(node=>node.getBoundingClientRect().bottom>bounds.bottom+scale).map(node=>node.textContent.slice(0,60)),
          unexpectedOverflow:Array.from(body.querySelectorAll('p,li,h1,h2')).filter(node=>!node.closest('.oversize-block')&&node.getBoundingClientRect().bottom>bounds.bottom+scale).map(node=>node.textContent.slice(0,60)),
          markers:Array.from(body.querySelectorAll('li'),node=>({text:node.textContent,marker:node.dataset.marker,continuation:node.dataset.continuation,content:getComputedStyle(node,'::before').content})),
          oversize:body.querySelectorAll('.oversize-block').length};
      });
      return {pages,text:pages.map(page=>page.text).join(''),oversize:pages.reduce((sum,page)=>sum+page.oversize,0),
        headers:Array.from(host.shadowRoot.querySelectorAll('.page-header'),node=>node.style.top),
        footers:Array.from(host.shadowRoot.querySelectorAll('.page-footer'),node=>node.style.bottom),
        paragraphStyles:Array.from(host.shadowRoot.querySelectorAll('.page-body p'),node=>({lineHeight:node.style.lineHeight,fontSize:getComputedStyle(node).fontSize})),batches};
    }finally{window.requestIdleCallback=idle;window.cancelIdleCallback=cancel;}
  },
  diagnose(){
    const body=host.shadowRoot.querySelector('.page-body');
    const original=body.firstElementChild.cloneNode(true);
    const rect=node=>{const r=node.getBoundingClientRect();return {top:r.top,bottom:r.bottom,height:r.height,width:r.width};};
    const bodyStyle=getComputedStyle(body);
    const metrics={zoom:host.style.zoom,body:rect(body),bodyHeight:bodyStyle.height,variants:[]};
    for(const count of [13,14,15,16])for(const trailing of [false,true]){
      const paragraph=original.cloneNode(false),span=original.querySelector('span').cloneNode(false);
      span.textContent=Array.from({length:count},(_,index)=>'Long'+String(index).padStart(3,'0')+' words').join('\\n')+(trailing?'\\n':'');
      paragraph.append(span);body.replaceChildren(paragraph);
      const range=document.createRange();range.selectNodeContents(span);
      metrics.variants.push({count,trailing,paragraph:rect(paragraph),span:rect(span),lineHeight:getComputedStyle(paragraph).lineHeight,
        fontSize:getComputedStyle(paragraph).fontSize,spanFont:getComputedStyle(span).fontSize,
        textRects:Array.from(range.getClientRects(),r=>({top:r.top,bottom:r.bottom,height:r.height,width:r.width}))});
    }
    return metrics;
  },
  inspect(){
    return Array.from(host.shadowRoot.querySelectorAll('.page-body'),body=>{
      const bounds=body.getBoundingClientRect();
      return {text:body.textContent,overflow:Array.from(body.querySelectorAll('p,img')).filter(node=>node.getBoundingClientRect().bottom>bounds.bottom+1).map(node=>node.tagName),
        images:Array.from(body.querySelectorAll('img'),node=>({complete:node.complete,naturalHeight:node.naturalHeight}))};
    });
  },
  dispose(){rendered?.dispose();return host.shadowRoot.childElementCount;}
};
`);
await build({ root: harness, configFile: false, base: '/word-pagination/', logLevel: 'warn', build: { target: 'es2022' } });
const dist = resolve(harness, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' };
const delayedImages = [];
const delayedFonts = [];
let fontBytes;
for (const fontPath of [process.env.WORD_TEST_FONT, '/System/Library/Fonts/Supplemental/Courier New.ttf', '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf', 'C:/Windows/Fonts/consola.ttf'].filter(Boolean)) {
  try { fontBytes = await readFile(fontPath); break; } catch { /* Try another installed font without adding a dependency. */ }
}
const server = createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  if (url.pathname === '/word-pagination/late-font.ttf') { delayedFonts.push(response); return; }
  if (url.pathname === '/word-pagination/release-font') {
    for (const font of delayedFonts.splice(0)) font.writeHead(200, { 'Content-Type': 'font/ttf', 'Cache-Control': 'max-age=3600' }).end(fontBytes);
    response.writeHead(200).end('released'); return;
  }
  if (url.pathname === '/word-pagination/late-image.svg') { delayedImages.push(response); return; }
  if (url.pathname === '/word-pagination/release-image') {
    for (const image of delayedImages.splice(0)) image.writeHead(200, { 'Content-Type': 'image/svg+xml', 'Cache-Control': 'max-age=3600' }).end('<svg xmlns="http://www.w3.org/2000/svg" width="160" height="160"><rect width="160" height="160" fill="blue"/></svg>');
    response.writeHead(200).end('released'); return;
  }
  if (!url.pathname.startsWith('/word-pagination/')) { response.writeHead(404).end(); return; }
  const path = resolve(dist, decodeURIComponent(url.pathname.slice('/word-pagination/'.length)) || 'index.html');
  if (!path.startsWith(dist + sep)) { response.writeHead(403).end(); return; }
  try { response.writeHead(200, { 'Content-Type': mime[extname(path)] || 'application/octet-stream' }); response.end(await readFile(path)); }
  catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 1000, height: 1000 }, deviceScaleFactor: 1 });
const errors = [], results = [];
page.on('pageerror', error => errors.push(error.message));
const setup = (extra = {}) => ({ page: { widthPt: 320, heightPt: 220, leftPt: 20, rightPt: 20, topPt: 20, bottomPt: 20, landscape: true }, header: [], firstHeader: [], evenHeader: [], footer: [], firstFooter: [], evenFooter: [], titlePage: false, pageNumbers: false, ...extra });
const text = (value, extra = {}) => ({ kind: 'paragraph', spacingBeforePt: 0, spacingAfterPt: 0, lineSpacing: { rule: 'exact', valuePt: 12 }, content: [{ kind: 'text', text: value, font: 'Arial', sizeHalfPt: 20 }], ...extra });
const lines = (count, prefix) => Array.from({ length: count }, (_, index) => `${prefix}${String(index).padStart(3, '0')} words`).join('\n');
const document = (blocks, config = setup()) => ({ blocks, setup: config, stats: { paragraphs: blocks.length, tables: 0, images: 0, runs: blocks.length } });
const render = (doc, scale = 1) => page.evaluate(({ doc, scale }) => window.wordPagination.render(doc, scale), { doc, scale });
const intact = (result, expected) => {
  assert.equal(result.text, expected, 'Text must retain exact order and characters across pages');
  assert.equal(result.oversize, 0, 'Flow text must use page fragments, without internal scrolling');
  assert.deepEqual(result.pages.flatMap(page => page.overflow), [], 'Flow fragments must fit the page body');
};
function sourceParagraphs(blocks) {
  return blocks.flatMap(block => {
    if (block.kind === 'table') return block.table.grid.flatMap(row => row.flatMap(slot => slot.kind === 'origin' ? sourceParagraphs(slot.cell.blocks) : []));
    if (block.kind === 'paragraph' || block.kind === 'heading' || block.kind === 'listItem') return [(block.content || []).map(inline => inline.text || '').join('')].filter(Boolean);
    return [];
  });
}
const check = async (name, run) => {
  try { const details = await run(); results.push({ name, passed: true, ...details }); console.log(`PASS ${name}`); }
  catch (error) { results.push({ name, passed: false, error: error.message }); console.error(`FAIL ${name}: ${error.message}`); }
};
try {
  await page.goto(`http://127.0.0.1:${server.address().port}/word-pagination/`);
  await page.waitForFunction(() => !!window.wordPagination);
  const longText = lines(90, 'Long');
  const longDocument = document([text(longText)]);
  await check('source-multiple-spacing-uses-font-line-metrics', async () => {
    const content = [{ kind: 'text', text: '中文 Ag\n中文 Ag', font: '宋体', sizeHalfPt: 24 }];
    const blocks = [
      text('', { content, lineSpacing: undefined, linePct: 1.5 }),
      text('', { content, lineSpacing: { rule: 'exact', valuePt: 18 }, linePct: 1.5 }),
    ];
    await render(document(blocks));
    const metrics = await page.locator('#host .page-body p').evaluateAll(paragraphs => paragraphs.map(p => {
      const run = p.firstElementChild, style = getComputedStyle(run);
      const canvas = document.createElement('canvas').getContext('2d');
      canvas.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const measured = canvas.measureText(run.textContent);
      return { height: p.getBoundingClientRect().height,
        natural: measured.fontBoundingBoxAscent + measured.fontBoundingBoxDescent,
        lineHeight: parseFloat(style.lineHeight) };
    }));
    assert.ok(Math.abs(metrics[0].lineHeight - metrics[0].natural * 1.5) < 0.1,
      'Source multiples must scale font ascent/descent rather than the em square');
    assert.ok(Math.abs(metrics[0].height - metrics[0].natural * 3) < 1);
    assert.ok(Math.abs(metrics[1].height - 48) < 1, 'Exact spacing must remain 18pt per line');
    return { metrics };
  });
  await check('soft-wrap-rich-text-graphemes', async () => {
    const unit = '中文软换行 e\u0301 👩‍💻 English wrapping words ';
    const runs = [
      { kind: 'text', text: unit.repeat(35), font: 'Arial', sizeHalfPt: 20 },
      { kind: 'text', text: unit.repeat(35), font: 'Arial', sizeHalfPt: 20, bold: true },
      { kind: 'text', text: unit.repeat(35), font: 'Arial', sizeHalfPt: 20, italic: true },
    ];
    const value = runs.map(run => run.text).join('');
    const result = await render(document([text('', { content: runs })])); intact(result, value);
    assert.ok(result.pages.length > 2);
    const boundaries = new Set([0, value.length]);
    for (const segment of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(value)) boundaries.add(segment.index);
    let offset = 0;
    for (const fragment of result.pages) { offset += fragment.text.length; assert.ok(boundaries.has(offset), 'Page boundary bisected a grapheme cluster'); }
    const styles = await page.evaluate(() => {
      const shadow=document.querySelector('#host').shadowRoot;
      return {bold:Array.from(shadow.querySelectorAll('.page-body .b'),node=>node.textContent).join(''),italic:Array.from(shadow.querySelectorAll('.page-body .i'),node=>node.textContent).join('')};
    });
    assert.equal(styles.bold, runs[1].text); assert.equal(styles.italic, runs[2].text);
    return { pages: result.pages.length, characters: value.length };
  });
  await check('paragraph-spans-pages', async () => {
    const result = await render(longDocument);
    await page.screenshot({ path: resolve(output, baseline ? 'paragraph-baseline.png' : 'paragraph-current.png'), fullPage: true });
    intact(result, longText);
    assert.ok(result.pages.length > 2, 'Long paragraph must span several pages');
    return { pages: result.pages.length };
  });
  await check('lists-and-nesting', async () => {
    const items = Array.from({ length: 35 }, (_, index) => text(`Item${index}`, { kind: 'listItem', list: { level: index % 3 === 1 ? 1 : 0, ordered: true, label: `${index + 1})` } }));
    items.splice(12, 0, text(lines(45, 'LongItem'), { kind: 'listItem', list: { level: 1, ordered: true, label: 'LONG)' } }));
    const result = await render(document(items)); intact(result, items.map(item => item.content[0].text).join(''));
    const markers = result.pages.flatMap(page => page.markers).filter(item => item.content !== 'none');
    assert.deepEqual(markers.map(item => item.marker), items.map(item => item.list.label), 'Each list marker must appear exactly once, in order');
    assert.ok(result.pages.length > 2); return { pages: result.pages.length, markers: markers.length };
  });
  await check('keep-with-next', async () => {
    const blocks = [text(lines(11, 'Fill')), text('HEADING', { kind: 'heading', level: 2, pagination: { keepNext: true, keepLines: true, widowControl: true } }), text(lines(5, 'Following'))];
    const result = await render(document(blocks)); intact(result, blocks.map(block => block.content[0].text).join(''));
    assert.equal(result.pages.findIndex(page => page.text.includes('HEADING')), result.pages.findIndex(page => page.text.includes('Following000')), 'Heading must travel with the following paragraph');
    return { pages: result.pages.length };
  });
  await check('keep-lines', async () => {
    const following = lines(5, 'Keep');
    const result = await render(document([text(lines(12, 'Fill')), text(following, { pagination: { keepNext: false, keepLines: true, widowControl: true } })]));
    assert.ok(result.pages.some(page => page.text.includes(following)), 'A paragraph that fits an empty page must stay together');
    intact(result, lines(12, 'Fill') + following); return { pages: result.pages.length };
  });
  await check('keep-heading-with-long-paragraph', async () => {
    const blocks = [text(lines(14, 'Fill')), text('HEADING', { kind: 'heading', level: 2,
      pagination: { keepNext: true, keepLines: true, widowControl: true } }), text(lines(90, 'Following'))];
    const result = await render(document(blocks));
    intact(result, blocks.map(block => block.content[0].text).join(''));
    assert.equal(result.pages.findIndex(page => page.text.includes('HEADING')),
      result.pages.findIndex(page => page.text.includes('Following000')), 'Heading must follow the start of a multi-page paragraph');
    return { pages: result.pages.length };
  });
  await check('page-break-before-list-item', async () => {
    const blocks = [text('ITEM1', { kind: 'listItem', list: { level: 0, ordered: true, label: '1)' } }),
      text('ITEM2', { kind: 'listItem', pageBreakBefore: true, list: { level: 0, ordered: true, label: '2)' } })];
    const result = await render(document(blocks));
    intact(result, 'ITEM1ITEM2');
    assert.deepEqual(result.pages.map(page => page.text), ['ITEM1', 'ITEM2']);
    return { pages: result.pages.length };
  });
  for (const [kind, count] of [['continuous', 1], ['nextPage', 2], ['evenPage', 2], ['oddPage', 3]]) {
    await check(`section-${kind}`, async () => {
      const result = await render(document([text('BEFORE'), { kind: 'sectionBreak', section: setup({ breakType: kind }) }, text('AFTER')]));
      intact(result, 'BEFOREAFTER'); assert.equal(result.pages.length, count);
      assert.ok(result.pages.at(-1).text.includes('AFTER')); return { pages: result.pages.length };
    });
  }
  await check('explicit-empty-pages', async () => {
    const result = await render(document([{ kind: 'pageBreak' }, { kind: 'pageBreak' }, text('AFTER')]));
    assert.deepEqual(result.pages.map(page => page.text), ['', '', 'AFTER']); return { pages: result.pages.length };
  });
  await check('page-break-before', async () => {
    const result = await render(document([text('BEFORE'), text('AFTER', { pageBreakBefore: true })]));
    assert.deepEqual(result.pages.map(page => page.text), ['BEFORE', 'AFTER']); return { pages: result.pages.length };
  });
  await check('zoom-invariance', async () => {
    const reference = await render(longDocument);
    if (process.env.WORD_PAGINATION_DIAGNOSTICS) {
      const diagnostics = await page.evaluate(() => window.wordPagination.diagnose());
      await writeFile(resolve(output, 'zoom-1-geometry.json'), JSON.stringify(diagnostics, null, 2));
    }
    for (const scale of [0.1, 4]) {
      const result = await render(longDocument, scale); intact(result, longText);
      if (process.env.WORD_PAGINATION_DIAGNOSTICS) {
        const diagnostics = await page.evaluate(() => window.wordPagination.diagnose());
        await writeFile(resolve(output, `zoom-${scale}-geometry.json`), JSON.stringify(diagnostics, null, 2));
      }
      assert.deepEqual(result.pages.map(page => page.text), reference.pages.map(page => page.text), `Page boundaries changed at zoom ${scale}`);
    }
    return { scales: [0.1, 1, 4], pages: reference.pages.length };
  });
  await check('widow-control', async () => {
    const value = lines(16, 'Widow');
    const result = await render(document([text(value, { pagination: { keepNext: false, keepLines: false, widowControl: true } })]));
    intact(result, value);
    assert.equal(result.pages.length, 2);
    for (const [index, page] of result.pages.entries()) {
      assert.ok(page.text.split('\n').filter(Boolean).length >= 2, `Page ${index + 1} must retain at least two lines`);
    }
    return { pages: result.pages.length };
  });
  await check('minimum-line-spacing', async () => {
    const block = text('LARGE TEXT\nSECOND LARGE LINE', { lineSpacing: { rule: 'atLeast', valuePt: 12 }, content: [{ kind: 'text', text: 'LARGE TEXT\nSECOND LARGE LINE', font: 'Arial', sizeHalfPt: 48 }] });
    await render(document([block]));
    const metrics = await page.evaluate(() => {
      const node=document.querySelector('#host').shadowRoot.querySelector('.page-body p');
      const range=document.createRange();range.selectNodeContents(node.firstElementChild);
      const rects=Array.from(range.getClientRects()).filter(r=>r.width>0);
      const tops=[...new Set(rects.map(r=>r.top))];
      return {height:node.getBoundingClientRect().height,lineStep:tops[1]-tops[0]};
    });
    assert.ok(metrics.lineStep >= 32, 'Minimum line spacing must grow to accommodate a 24pt font');
    assert.ok(metrics.height >= 64, 'Two 24pt lines cannot be compressed into 12pt line boxes');
    return metrics;
  });
  await check('late-image-repagination', async () => {
    const filler=lines(8,'ImageFill');
    const pendingImage=page.waitForRequest(request=>request.url().endsWith('/late-image.svg'));
    const initial=await render(document([text(filler),{kind:'image',src:'./late-image.svg',alt:'Delayed image'},text('AFTER-LATE-IMAGE')]));
    assert.equal(initial.pages.length,1,'Image must still be pending before its natural dimensions arrive');
    await pendingImage;
    await page.evaluate(()=>fetch('./release-image'));
    await page.waitForFunction(()=>{
      const root=document.querySelector('#host').shadowRoot;
      return root.querySelectorAll('.page-body').length>=2&&Array.from(root.querySelectorAll('img')).every(image=>image.complete&&image.naturalHeight>0);
    },undefined,{timeout:5000});
    const pages=await page.evaluate(()=>window.wordPagination.inspect());
    assert.equal(pages.map(page=>page.text).join(''),filler+'AFTER-LATE-IMAGE');
    assert.deepEqual(pages.flatMap(page=>page.overflow),[],'Late image must trigger pagination before content is clipped');
    return {before:initial.pages.length,after:pages.length};
  });
  await check('late-font-repagination', async () => {
    if (!fontBytes) return { skipped: 'Set WORD_TEST_FONT to an installed TTF font to exercise delayed font loading' };
    await page.evaluate(()=>document.fonts.add(new FontFace('WordPaginationDelayed','url(./late-font.ttf)')));
    const pendingFont=page.waitForRequest(request=>request.url().endsWith('/late-font.ttf'));
    const value='iiiiiiii soft wrapping text '.repeat(180);
    const doc=document([text(value,{content:[{kind:'text',text:value,font:'WordPaginationDelayed',sizeHalfPt:20}]})]);
    const initial=await render(doc);
    await pendingFont;
    await page.evaluate(()=>fetch('./release-font'));
    await page.waitForFunction(()=>document.fonts.check('10pt WordPaginationDelayed'),undefined,{timeout:5000});
    // A font event queues a reflow frame; inspect after that frame has completed.
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const loaded=await page.evaluate(()=>window.wordPagination.inspect());
    const reference=await render(doc);
    intact(reference,value);
    assert.equal(loaded.map(page=>page.text).join(''),value,'Font replacement must retain all text');
    assert.deepEqual(loaded.map(page=>page.text),reference.pages.map(page=>page.text),'Delayed font pagination must match pagination with the font already available');
    assert.deepEqual(loaded.flatMap(page=>page.overflow),[],'Font replacement must not clip content');
    return {before:initial.pages.length,after:loaded.length};
  });
  await check('section-distances-and-line-spacing', async () => {
    const result = await render(document([text('LINE')], setup({ headerDistancePt: 34, footerDistancePt: 40 })));
    assert.deepEqual(result.headers, ['34pt']); assert.deepEqual(result.footers, ['40pt']);
    assert.equal(result.paragraphStyles[0].lineHeight, '12pt'); return {};
  });
  if (!process.argv.includes('--synthetic-only')) {
    const wasmDirectory = resolve(root, 'packages/viewer-word/src/wasm');
    const engine = await import(pathToFileURL(resolve(wasmDirectory, 'word_view_rs.js')).href);
    engine.initSync({ module: await readFile(resolve(wasmDirectory, 'word_view_rs_bg.wasm')) });
    if (process.env.WORD_LIST_DOCX) await check('external-document-bullet-markers', async () => {
      const doc = engine.toDocument(await readFile(resolve(process.env.WORD_LIST_DOCX)));
      const bullets = doc.blocks.filter(block => block.list && !block.list.ordered);
      assert.ok(bullets.length > 0, 'The supplied document must contain unordered lists');
      const result = await render(doc);
      const markers = await page.evaluate(() => Array.from(
        document.querySelector('#host').shadowRoot.querySelectorAll('ul > li[data-marker]:not([data-continuation])'),
        node => ({ marker: node.dataset.marker, text: node.textContent, font: getComputedStyle(node, '::before').fontFamily }),
      ));
      const prefix = process.env.WORD_LIST_ARTIFACT_PREFIX || 'external-lists';
      const firstList = page.locator('#host .list-run').filter({ has: page.locator('ul > li[data-marker]') }).first();
      await firstList.screenshot({ path: resolve(output, prefix + '.png') });
      await firstList.locator('xpath=ancestor::section').screenshot({ path: resolve(output, prefix + '-page.png') });
      await writeFile(resolve(output, prefix + '.json'), JSON.stringify({ pages: result.pages.length, sourceBullets: bullets.length, markers }, null, 2));
      assert.deepEqual(markers.map(marker => marker.marker), bullets.map(block => block.list.label), 'Every source bullet must render exactly once, in order');
      assert.ok(markers.every(marker => !/[\uf000-\uf0ff]/u.test(marker.marker)), 'Legacy symbol-font codes must be decoded before browser rendering');
      assert.deepEqual(result.pages.flatMap(page => page.unexpectedOverflow), [], 'The document must stay inside its page bodies');
      return { pages: result.pages.length, bullets: markers.length };
    });
    for (const name of ['styles-word.docx', 'req.docx', 'manual.docx']) {
      await check(`real-${name}`, async () => {
        const doc = engine.toDocument(await readFile(resolve(root, 'apps/word-view/public/samples', name)));
        const paragraphs = sourceParagraphs(doc.blocks);
        const started = performance.now();
        const result = await render(doc);
        const logical = await page.evaluate(collectWordFlowContent);
        let cursor = 0;
        for (const [index, value] of paragraphs.entries()) {
          const next = logical.text.indexOf(value, cursor);
          assert.ok(next >= 0, `Source paragraph ${index} is missing or out of order: ${value.slice(0,70)}`);
          cursor = next + value.length;
        }
        assert.ok(result.pages.length > 0, 'Real document must mount at least one page');
        if (!baseline) assert.deepEqual(result.pages.flatMap(page => page.unexpectedOverflow), [], 'Flow text outside a known oversized table/image must fit the page');
        const prefix = name.replace(/\.docx$/, '') + (baseline ? '-baseline' : '-current');
        const pages = page.locator('#host').locator('.page');
        await pages.first().screenshot({ path: resolve(output, prefix + '-first.png') });
        if (result.pages.length > 1) await pages.last().screenshot({ path: resolve(output, prefix + '-last.png') });
        return { pages: result.pages.length, paragraphs: paragraphs.length, textCharacters: result.text.length, oversize: result.oversize,
          overflow: result.pages.flatMap(page => page.overflow).length, unexpectedOverflow: result.pages.flatMap(page => page.unexpectedOverflow).length,
          batches: result.batches, renderMs: Math.round(performance.now() - started) };
      });
    }
  }
  await check('dispose', async () => { assert.equal(await page.evaluate(() => window.wordPagination.dispose()), 0); return {}; });
} finally {
  await writeFile(resolve(output, baseline ? 'baseline-results.json' : 'results.json'), JSON.stringify({ baseline, results, errors }, null, 2));
  await browser.close(); await new Promise(done => server.close(done));
}
assert.deepEqual(errors, [], 'Browser errors');
assert.ok(results.every(result => result.passed), `${results.filter(result => !result.passed).length} pagination regressions failed`);
console.log(`PASS ${results.length} Word pagination regressions`);
