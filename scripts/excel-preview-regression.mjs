import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cp, mkdir, mkdtemp, readFile, realpath, symlink, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { arch, cpus, platform, release, tmpdir } from 'node:os';
import { dirname, extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { describeFixture, fixtureNames } from './excel-preview-fixtures.mjs';
import { createCompatibilityWorkbook } from './excel-preview-xlsx.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output');
const baselineOnly = process.argv.includes('--baseline-only');
const baselineRevision = execFileSync('git', ['rev-parse', process.env.EXCEL_BASELINE_REF || '0ac7d51'], { cwd: root, encoding: 'utf8' }).trim();
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const harness = await realpath(await mkdtemp(resolve(tmpdir(), 'excel-preview-')));
const thresholds = { cellDom: 6000, largeFirstVisibleMs: 2000, switchMs: 1000, smallRatio: 1.2, smallSlackMs: 20, baselineCellLimit: 20000 };
await mkdir(output, { recursive: true });
await symlink(resolve(root, 'packages/viewer/node_modules'), resolve(harness, 'node_modules'), 'dir');
const baselineFiles = execFileSync('git', ['ls-tree', '-r', '--name-only', baselineRevision, 'packages/viewer-excel/src/core'], { cwd: root, encoding: 'utf8' }).trim().split('\n');
assert.ok(baselineFiles.some(file => file.endsWith('/render.ts')), 'Baseline commit must contain the SDK Excel renderer');
for (const file of baselineFiles) {
  const destination = resolve(harness, 'baseline', file.split('/').at(-1));
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, execFileSync('git', ['show', `${baselineRevision}:${file}`], { cwd: root }));
}
await cp(resolve(root, 'scripts/excel-preview-fixtures.mjs'), resolve(harness, 'fixtures.mjs'));
await cp(resolve(root, 'apps/excel-view/public/samples'), resolve(harness, 'public/samples'), { recursive: true });
await writeFile(resolve(harness, 'public/samples/generated-compatibility.xlsx'), createCompatibilityWorkbook());
await writeFile(resolve(harness, 'index.html'), `<!doctype html><meta charset="utf-8"><title>Excel preview benchmark</title>
<style>html,body{margin:0;background:white;font-family:Arial,sans-serif}#host{width:1400px;margin:0}</style>
<div id="host"></div><script type="module" src="/main.js"></script>`);
await writeFile(resolve(harness, 'main.js'), `
import {renderExcelInto} from './baseline/render.ts';
import {createExcelAdapter} from '@office-view-suite/viewer-excel';
import {createFixture} from './fixtures.mjs';
const host=document.querySelector('#host');
let rendered,adapter;
const frames=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
const active=()=>host.shadowRoot?.querySelector('.sheet:not([hidden])');
const viewport=()=>active()?.querySelector('.grid-wrap');
function snapshot(){
  const wrap=viewport();
  const view=wrap?.getBoundingClientRect();
  const intersects=el=>{const r=el.getBoundingClientRect();return view&&r.right>view.left&&r.left<view.right&&r.bottom>view.top&&r.top<view.bottom;};
  return {cellDom:host.shadowRoot?.querySelectorAll('.cell').length??0,sheetDom:host.shadowRoot?.querySelectorAll('.sheet').length??0,
    activeSheets:host.shadowRoot?.querySelectorAll('.sheet:not([hidden])').length??0,
    tabs:Array.from(host.shadowRoot?.querySelectorAll('.sheet-tab')??[],x=>x.textContent),
    text:active()?.textContent??'',scrollLeft:wrap?.scrollLeft??0,scrollTop:wrap?.scrollTop??0,
    scrollWidth:wrap?.scrollWidth??0,scrollHeight:wrap?.scrollHeight??0,clientWidth:wrap?.clientWidth??0,clientHeight:wrap?.clientHeight??0,
    images:Array.from(active()?.querySelectorAll('img')??[]).filter(intersects).length,
    shapes:Array.from(active()?.querySelectorAll('.sheet-shape')??[]).filter(intersects).length,
    jsHeapSnapshotBytes:performance.memory?.usedJSHeapSize??null};
}
function clear(){rendered?.dispose();adapter?.destroy();rendered=undefined;adapter=undefined;host.style.zoom='1';host.style.width='1400px';}
window.excelBenchmark={
  async render(name,variant){
    clear();const doc=createFixture(name);await document.fonts.ready;
    const start=performance.now();
    if(variant==='baseline')rendered=renderExcelInto(host,doc);
    else {adapter=createExcelAdapter();rendered=adapter.render(host,doc);}
    const mountMs=performance.now()-start;await frames();
    return {mountMs,firstVisibleMs:performance.now()-start,...snapshot()};
  },
  async scroll(left,top){const wrap=viewport();wrap.scrollLeft=left==='end'?wrap.scrollWidth:left;wrap.scrollTop=top==='end'?wrap.scrollHeight:top;wrap.dispatchEvent(new Event('scroll'));await frames();return snapshot();},
  async sheet(index){const start=performance.now();host.shadowRoot.querySelectorAll('.sheet-tab')[index].click();await frames();return {switchMs:performance.now()-start,...snapshot()};},
  async resize(width,zoom){host.style.width=width+'px';host.style.zoom=String(zoom);rendered?.resize?.();await frames();return snapshot();},
  async inspectCell(row,col){
    const found=Array.from(active().querySelectorAll('.cell')).find(el=>Number(el.dataset.row)===row&&Number(el.dataset.col)===col);
    if(!found)return null;const rect=found.getBoundingClientRect();const view=viewport().getBoundingClientRect();
    return {text:found.textContent,width:rect.width,height:rect.height,left:rect.left,top:rect.top,bottom:rect.bottom,
      viewportTop:view.top,viewportBottom:view.bottom};
  },
  inspectDrawings(){
    const view=viewport().getBoundingClientRect();
    return Array.from(active().querySelectorAll('img,.sheet-shape'),node=>{const r=node.getBoundingClientRect();return {
      right:r.right,bottom:r.bottom,width:r.width,height:r.height,visibleWidth:Math.max(0,Math.min(r.right,view.right)-Math.max(r.left,view.left)),
      visibleHeight:Math.max(0,Math.min(r.bottom,view.bottom)-Math.max(r.top,view.top)),viewportRight:view.right,viewportBottom:view.bottom};});
  },
  denseGuard(){
    clear();const doc=createFixture('medium'),sheet=doc.sheets[0];sheet.highestCol=200;sheet.cells=[];
    sheet.colWidths=Array.from({length:200},(_,i)=>({col:i+1,width:.1}));
    sheet.rowHeights=Array.from({length:1000},(_,i)=>({row:i+1,height:2}));
    adapter=createExcelAdapter();
    try{rendered=adapter.render(host,doc);return {code:'UNEXPECTED_SUCCESS'};}
    catch(error){return {limited:/viewport exceeds the preview limit/.test(error.message),children:host.shadowRoot.childElementCount};}
  },
  async real(name){
    clear();const start=performance.now();adapter=createExcelAdapter();const response=await fetch('./samples/'+name);
    if(!response.ok)throw new Error('Sample fetch failed: '+name);const bytes=await response.arrayBuffer();const loaded=performance.now();
    const parsed=await adapter.open({name,bytes});const parsedAt=performance.now();rendered=adapter.render(host,parsed.document);await frames();
    return {name,kind:'real-XLSX-cold-adapter',bytes:bytes.byteLength,fetchMs:loaded-start,openMs:parsedAt-loaded,
      firstVisibleMs:performance.now()-start,document:parsed.document,...snapshot()};
  },
  async dispose(){const old=viewport();if(old){old.scrollTop+=100;old.dispatchEvent(new Event('scroll'));}clear();old?.dispatchEvent(new Event('scroll'));await frames();return {children:host.shadowRoot?.childElementCount??0};}
};
`);
await build({ root: harness, configFile: false, base: '/excel-regression/', logLevel: 'warn', build: { target: 'es2022' } });
const dist = resolve(harness, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  if (!path.startsWith('/excel-regression/')) { res.writeHead(404).end(); return; }
  const file = resolve(dist, path.slice('/excel-regression/'.length) || 'index.html');
  if (!file.startsWith(dist + sep)) { res.writeHead(403).end(); return; }
  try { res.writeHead(200, { 'Content-Type': mime[extname(file)] || 'application/octet-stream' }); res.end(await readFile(file)); }
  catch { res.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [], cases = [], interactions = [], files = [];
const report = {
  baselineRevision, mode: baselineOnly ? 'baseline-only' : 'comparison',
  currentBundleSha256: createHash('sha256').update(await readFile(resolve(root, 'packages/viewer-excel/dist/index.js'))).digest('hex'),
  environment: { platform: platform(), release: release(), arch: arch(), cpu: cpus()[0]?.model, node: process.version, browser: browser.version(), viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 },
  measurement: { warmups: 1, repetitions: 5, firstVisible: 'render call through two animation frames', memory: 'Optional Chromium JS heap snapshot, not process/WASM peak memory', realFiles: 'fresh adapter for each file; browser cache may already be warm' },
  thresholds, cases, interactions, files, errors,
};
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
page.on('pageerror', error => errors.push(error.message));
page.setDefaultTimeout(10000);
const url = `http://127.0.0.1:${server.address().port}/excel-regression/`;
// A synchronous DOM storm cannot honor a browser-side timer. Enforce a Node deadline.
async function bounded(work) {
  let timer;
  try { return await Promise.race([work, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Browser operation exceeded 10 seconds')), 10000); })]); }
  finally { clearTimeout(timer); }
}
const summarize = values => ({ median: [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)], max: Math.max(...values) });
const concise = ({ text, ...value }) => value;
const domBound = value => assert.ok(value.cellDom <= thresholds.cellDom, `cell DOM ${value.cellDom} exceeds ${thresholds.cellDom}`);
try {
  await page.goto(url);
  await page.waitForFunction(() => !!window.excelBenchmark);
  for (const name of fixtureNames) {
    const fixture = describeFixture(name);
    const result = { ...fixture };
    for (const variant of baselineOnly ? ['baseline'] : ['baseline', 'current']) {
      if (variant === 'baseline' && fixture.possibleCells > thresholds.baselineCellLimit) {
        result.baseline = { skipped: 'Safety bound: full-grid baseline exceeds 20,000 possible cells; no timing is claimed' };
        continue;
      }
      const samples = [];
      for (let run = 0; run < 6; run++) {
        const value = await bounded(page.evaluate(({ name, variant }) => window.excelBenchmark.render(name, variant), { name, variant }));
        if (variant === 'current') {
          domBound(value);
          assert.equal(value.sheetDom, 1, `${name}: only one sheet may be mounted`);
          if (fixture.possibleCells > 20000) assert.ok(value.firstVisibleMs <= thresholds.largeFirstVisibleMs, `${name}: first visible ${value.firstVisibleMs}ms`);
        }
        if (run > 0) samples.push(concise(value));
      }
      result[variant] = { mountMs: summarize(samples.map(x => x.mountMs)), firstVisibleMs: summarize(samples.map(x => x.firstVisibleMs)), cellDom: summarize(samples.map(x => x.cellDom)), samples };
    }
    cases.push(result);
    if (!baselineOnly && name === 'small') {
      assert.ok(result.current.firstVisibleMs.median <= result.baseline.firstVisibleMs.median * thresholds.smallRatio + thresholds.smallSlackMs, 'Small-sheet first-visible regression');
    }
    console.log(JSON.stringify({ fixture: name, baseline: result.baseline?.firstVisibleMs ?? result.baseline?.skipped, current: result.current?.firstVisibleMs, dom: result.current?.cellDom }));
  }
  if (!baselineOnly) {
    await page.evaluate(() => window.excelBenchmark.render('multiple-sheets', 'current'));
    const before = await page.evaluate(() => window.excelBenchmark.scroll(350, 600));
    const second = await page.evaluate(() => window.excelBenchmark.sheet(1));
    domBound(second); assert.equal(second.sheetDom, 1); assert.ok(second.switchMs <= thresholds.switchMs);
    await page.evaluate(() => window.excelBenchmark.scroll(120, 300));
    const returned = await page.evaluate(() => window.excelBenchmark.sheet(0));
    assert.equal(returned.scrollTop, before.scrollTop); assert.equal(returned.scrollLeft, before.scrollLeft);
    assert.ok(returned.switchMs <= thresholds.switchMs);
    const secondAgain = await page.evaluate(() => window.excelBenchmark.sheet(1));
    assert.equal(secondAgain.scrollTop, 300); assert.equal(secondAgain.scrollLeft, 120);
    interactions.push({ test: 'sheet-scroll-restoration', before: concise(before), second: concise(second), returned: concise(returned) });

    for (const name of ['sparse-100k', 'wide']) {
      await page.evaluate(name => window.excelBenchmark.render(name, 'current'), name);
      const end = await page.evaluate(() => window.excelBenchmark.scroll('end', 'end'));
      domBound(end); assert.ok(end.text.includes('DOCUMENT-END'), `${name}: last cell is missing after scrolling`);
      const resized = await page.evaluate(() => window.excelBenchmark.resize(900, 1.5));
      domBound(resized);
      const scaledEnd = await page.evaluate(() => window.excelBenchmark.scroll('end', 'end'));
      assert.ok(scaledEnd.text.includes('DOCUMENT-END'), `${name}: last cell missing after scale/resize`);
      await page.screenshot({ path: resolve(output, `excel-${name}-end.png`), fullPage: true });
      interactions.push({ test: `${name}-end-and-resize`, end: concise(end), scaledEnd: concise(scaledEnd) });
    }

    await page.evaluate(() => window.excelBenchmark.render('merged-overlays', 'current'));
    const frozenBefore = await page.evaluate(() => window.excelBenchmark.inspectCell(1, 1));
    const columnBefore = await page.evaluate(() => window.excelBenchmark.inspectCell(1, 7));
    const merge = await page.evaluate(() => window.excelBenchmark.scroll(0, 900));
    domBound(merge); assert.ok(merge.text.includes('CROSS-VIEWPORT-MERGE'), 'Merge whose anchor is above the viewport must remain rendered');
    assert.ok(merge.images > 0, 'Intersecting image is missing');
    assert.ok(merge.shapes > 0, 'Intersecting shape is missing');
    const frozenAfter = await page.evaluate(() => window.excelBenchmark.inspectCell(1, 1));
    const mergeBounds = await page.evaluate(() => window.excelBenchmark.inspectCell(10, 3));
    assert.ok(frozenBefore && frozenAfter, 'Frozen corner is missing');
    assert.ok(Math.abs(frozenBefore.top - frozenAfter.top) <= 1, 'Frozen row moves while scrolling');
    assert.ok(mergeBounds && mergeBounds.bottom > mergeBounds.viewportTop && mergeBounds.top < mergeBounds.viewportBottom, 'Merged cell is not visibly intersecting its viewport');
    await page.screenshot({ path: resolve(output, 'excel-cross-viewport.png') });
    interactions.push({ test: 'cross-viewport-merge-and-overlays', ...concise(merge) });
    await page.evaluate(() => window.excelBenchmark.scroll(0, 900 * 26));
    const columnAfter = await page.evaluate(() => window.excelBenchmark.inspectCell(1, 7));
    assert.ok(columnBefore && columnAfter, 'Frozen row must retain the measured column');
    assert.ok(Math.abs(columnBefore.width - columnAfter.width) <= 1, 'Offscreen long text changes column width during scrolling');

    await page.evaluate(() => window.excelBenchmark.render('huge-merge', 'current'));
    const huge = await page.evaluate(() => window.excelBenchmark.scroll(0, 'end'));
    domBound(huge); assert.ok(huge.text.includes('HUGE-MERGE'), 'Huge merge disappears at its bottom');
    interactions.push({ test: 'huge-merge-bottom', ...concise(huge) });

    await page.evaluate(() => window.excelBenchmark.render('trailing-overlays', 'current'));
    const tail = await page.evaluate(() => window.excelBenchmark.scroll('end', 'end'));
    domBound(tail);
    assert.ok(tail.scrollTop > 5000 * 26, 'Trailing drawing viewport must reach beyond the last row');
    const drawings = await page.evaluate(() => window.excelBenchmark.inspectDrawings());
    assert.equal(drawings.length, 2, 'Both drawings must remain mounted beyond the last row/column');
    for (const drawing of drawings) {
      assert.ok(drawing.right <= drawing.viewportRight + 2 && drawing.bottom <= drawing.viewportBottom + 2, 'Trailing drawing edge is unreachable');
      assert.ok(drawing.visibleWidth >= 450 && drawing.visibleHeight >= 90, 'Trailing drawing is clipped to its anchor cell');
    }
    interactions.push({ test: 'drawing-extents-beyond-last-cell', ...concise(tail), drawings });
    await page.screenshot({ path: resolve(output, 'excel-trailing-drawings.png'), fullPage: true });
    // Direct adapters throw render errors; the facade's separately tested boundary maps them to RENDER_FAILED.
    assert.deepEqual(await page.evaluate(() => window.excelBenchmark.denseGuard()), { limited: true, children: 0 });
    interactions.push({ test: 'dense-viewport-fails-with-cleanup', limited: true });
  }

  // Expected fields come from the checked-in XLSX XML, not the other renderer's output.
  for (const name of ['handmade-merged.xlsx', 'sheet.xlsx', 'styles-excel.xlsx', 'generated-compatibility.xlsx']) {
    const result = await bounded(page.evaluate(name => window.excelBenchmark.real(name), name));
    const sheets = result.document.sheets;
    const value = (row, col) => sheets[0].cells.find(cell => cell.row === row && cell.col === col);
    if (name === 'handmade-merged.xlsx') {
      assert.deepEqual(sheets[0].merges, ['A1:B1', 'A2:A3']);
      assert.equal(value(1, 1)?.value, 'Merged across');
      assert.equal(value(2, 3)?.value, '3.5');
    } else if (name === 'sheet.xlsx') {
      assert.deepEqual(sheets.map(sheet => sheet.name), ['Values', 'Merged Grid']);
      assert.equal(value(2, 2)?.value, '0.155');
      assert.ok(value(2, 2)?.numfmt?.includes('%'), 'Percent format was lost');
    } else if (name === 'styles-excel.xlsx') {
      assert.equal(sheets[0].name, '样式矩阵');
      assert.ok(sheets[0].cells.some(cell => cell.bold), 'Bold formatting was lost');
      assert.ok(sheets[0].cells.some(cell => cell.fill), 'Cell fill was lost');
    } else {
      assert.equal(sheets[0].name, 'Compatibility');
      assert.equal(value(2, 1)?.formula, 'SUM(1,2)');
      assert.equal(value(2, 1)?.value, '3', 'Formula cached value was lost');
      assert.deepEqual(value(1, 1)?.richText?.map(run => run.text), ['Bold', ' plain']);
      assert.equal(value(1, 1)?.richText?.[0].bold, true);
      assert.equal(sheets[0].images.length, 1);
      const image = sheets[0].images[0];
      assert.deepEqual([image.col, image.row, image.widthPx, image.heightPx], [2, 2, 100, 50]);
      assert.ok(image.data.startsWith('data:image/png;base64,'));
    }
    const { document, text, ...measurements } = result;
    files.push({ ...measurements, sheets: sheets.map(sheet => ({ name: sheet.name, cells: sheet.cells.length, merges: sheet.merges.length })), independentAssertions: 'passed' });
  }
  if (!baselineOnly) await page.evaluate(() => window.excelBenchmark.render('sparse-100k', 'current'));
  assert.deepEqual(await page.evaluate(() => window.excelBenchmark.dispose()), { children: 0 });
  assert.deepEqual(errors, [], 'Unexpected browser errors');
  report.passed = true;
} catch (error) {
  report.passed = false;
  report.failure = { message: error.message, stack: error.stack };
  throw error;
} finally {
  await writeFile(resolve(output, baselineOnly ? 'excel-preview-baseline.json' : 'excel-preview-results.json'), JSON.stringify(report, null, 2));
  await browser.close();
  await new Promise(done => server.close(done));
}
console.log(`Excel preview ${baselineOnly ? 'baseline' : 'regression'} passed: ${cases.length} DTO cases, ${files.length} XLSX files, ${interactions.length} interactions`);
