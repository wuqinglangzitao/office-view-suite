import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cp, mkdir, mkdtemp, readFile, realpath, symlink, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output/playwright/visual-regression');
const { consumer } = JSON.parse(await readFile(resolve(root, 'output/packed-consumer.json'), 'utf8'));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const harness = await realpath(await mkdtemp(resolve(tmpdir(), 'office-sdk-visual-')));
const baselineRevision = execFileSync('git', ['rev-parse', process.env.SDK_BASELINE_REF || 'ee6dbbe'], { cwd: root, encoding: 'utf8' }).trim();
const samples = [
  ['word', 'rich.docx'], ['word', 'styles-word.docx'],
  ['excel', 'rich.xlsx'], ['excel', 'styles-excel.xlsx'],
  ['ppt', 'arch.pptx'], ['ppt', 'styles-ppt.pptx'],
];
await mkdir(output, { recursive: true });
await symlink(resolve(consumer, 'node_modules'), resolve(harness, 'node_modules'), 'dir');
for (const kind of ['word', 'excel', 'ppt']) {
  const paths = execFileSync('git', ['ls-tree', '-r', '--name-only', baselineRevision, `apps/${kind}-view/src/core`], { cwd: root, encoding: 'utf8' }).trim().split('\n');
  for (const path of paths) {
    const destination = resolve(harness, 'baseline', kind, path.split('/').at(-1));
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, execFileSync('git', ['show', `${baselineRevision}:${path}`], { cwd: root }));
  }
}
await mkdir(resolve(harness, 'public/samples'), { recursive: true });
for (const [kind, name] of samples) await cp(resolve(root, `apps/${kind}-view/public/samples/${name}`), resolve(harness, 'public/samples', name));
await writeFile(resolve(harness, 'index.html'), `<!doctype html><meta charset="utf-8"><title>Office SDK visual regression</title>
<style>html,body{margin:0;padding:0;background:white} .host{position:absolute;left:0;top:0;width:1100px} #sdk{visibility:hidden}</style>
<div id="baseline" class="host"></div><div id="sdk" class="host"></div><script type="module" src="/main.js"></script>`);
await writeFile(resolve(harness, 'main.js'), `
import {renderWordInto} from './baseline/word/render.ts';
import {renderExcelInto} from './baseline/excel/render.ts';
import {renderPptInto} from './baseline/ppt/render.ts';
import {createWordAdapter} from '@office-view-suite/viewer-word';
import {createExcelAdapter} from '@office-view-suite/viewer-excel';
import {createPptAdapter} from '@office-view-suite/viewer-ppt';
const renderers = {word:renderWordInto,excel:renderExcelInto,ppt:renderPptInto};
const factories = {word:createWordAdapter,excel:createExcelAdapter,ppt:createPptAdapter};
const baseline = document.querySelector('#baseline'), sdk = document.querySelector('#sdk');
let adapter, oldRender, newRender;
window.regression = {
  async open(kind,name) {
    oldRender?.dispose(); newRender?.dispose(); adapter?.destroy();
    adapter=factories[kind]();
    const response=await fetch('./samples/'+name);
    if(!response.ok) throw new Error('Sample fetch failed: '+response.status);
    const parsed=await adapter.open({name,bytes:await response.arrayBuffer()});
    oldRender=renderers[kind](baseline,parsed.document);
    newRender=adapter.render(sdk,parsed.document);
    await document.fonts.ready;
    await Promise.all([baseline,sdk].flatMap(host=>Array.from(host.shadowRoot.querySelectorAll('img')).map(image=>image.decode().catch(()=>{}))));
    return {stats:parsed.stats,warnings:parsed.warnings};
  },
  show(side) { baseline.style.visibility=side==='baseline'?'visible':'hidden'; sdk.style.visibility=side==='sdk'?'visible':'hidden'; },
  sheet(index) { for(const host of [baseline,sdk]) host.shadowRoot.querySelectorAll('.sheet-tab')[index].click(); },
  close() {oldRender?.dispose(); newRender?.dispose(); adapter?.destroy();}
};
`);
await build({ root: harness, configFile: false, base: '/regression/', logLevel: 'warn', build: { target: 'es2022' } });

const dist = resolve(harness, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer(async (req, res) => {
  const url = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  if (!url.startsWith('/regression/')) { res.writeHead(404).end(); return; }
  const path = resolve(dist, url.slice('/regression/'.length) || 'index.html');
  if (path !== dist && !path.startsWith(dist + sep)) { res.writeHead(403).end(); return; }
  try { const content = await readFile(path); res.writeHead(200, {'Content-Type':mime[extname(path)] || 'application/octet-stream'}); res.end(content); }
  catch { res.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({channel:'chrome',headless:true});
const page = await browser.newPage({viewport:{width:1200,height:1000},deviceScaleFactor:1});
const browserErrors = [], failedRequests = [], results = [];
page.on('pageerror', error => browserErrors.push(error.message));
page.on('requestfailed', request => failedRequests.push({url:request.url(),error:request.failure()?.errorText}));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
try {
  await page.goto(`http://127.0.0.1:${server.address().port}/regression/`);
  await page.waitForFunction(() => !!window.regression);
  for (const [kind, name] of samples) {
    const result = await page.evaluate(async ({kind,name}) => window.regression.open(kind,name), {kind,name});
    // Word batches in idle callbacks. Wait for both complete DOMs to settle, not just the first page.
    await page.waitForFunction(() => {
      const value = ['baseline','sdk'].map(id => document.querySelector('#'+id).shadowRoot.innerHTML).join('');
      const state = window.regressionStable;
      if (!state || state.value!==value) {window.regressionStable={value,since:performance.now()};return false;}
      return performance.now()-state.since>800;
    }, undefined, {timeout:20000,polling:100});
    const selector = kind==='word'?'.page':kind==='excel'?'.sheet':'.slide';
    const oldUnits = page.locator('#baseline').locator(selector), newUnits = page.locator('#sdk').locator(selector);
    const count = await oldUnits.count();
    assert.ok(count>0, name+' has no rendered pages/sheets');
    assert.equal(await newUnits.count(),count,name+' page/sheet count changed');
    const comparisons = [];
    for (let index=0;index<count;index++) {
      if(kind==='excel') await page.evaluate(index=>window.regression.sheet(index),index);
      const prefix = name.replace(/\.[^.]+$/,'')+'-'+String(index+1).padStart(3,'0');
      await page.evaluate(()=>window.regression.show('baseline'));
      const before = await oldUnits.nth(index).screenshot({path:resolve(output,prefix+'-baseline.png'),animations:'disabled',caret:'hide',scale:'css'});
      await page.evaluate(()=>window.regression.show('sdk'));
      const after = await newUnits.nth(index).screenshot({path:resolve(output,prefix+'-sdk.png'),animations:'disabled',caret:'hide',scale:'css'});
      const byteExact = before.equals(after);
      const pixels = byteExact ? {changedPixels:0} : await page.evaluate(async ({before,after}) => {
        const decode = data => new Promise((resolve,reject) => {const image=new Image();image.onload=()=>resolve(image);image.onerror=reject;image.src='data:image/png;base64,'+data;});
        const [left,right] = await Promise.all([decode(before),decode(after)]);
        if(left.width!==right.width || left.height!==right.height) return {dimensionMismatch:true,baseline:[left.width,left.height],sdk:[right.width,right.height]};
        const canvas=document.createElement('canvas');canvas.width=left.width;canvas.height=left.height;
        const context=canvas.getContext('2d');context.drawImage(left,0,0);const a=context.getImageData(0,0,canvas.width,canvas.height).data;
        context.clearRect(0,0,canvas.width,canvas.height);context.drawImage(right,0,0);const b=context.getImageData(0,0,canvas.width,canvas.height).data;
        let changedPixels=0,maxChannelDelta=0,sumDelta=0;
        const bounds={left:canvas.width,top:canvas.height,right:0,bottom:0};
        for(let pixel=0;pixel<a.length;pixel+=4){let changed=false;for(let channel=0;channel<4;channel++){const delta=Math.abs(a[pixel+channel]-b[pixel+channel]);sumDelta+=delta;maxChannelDelta=Math.max(maxChannelDelta,delta);changed ||= delta!==0;}if(changed){changedPixels++;const x=(pixel/4)%canvas.width,y=Math.floor(pixel/4/canvas.width);bounds.left=Math.min(bounds.left,x);bounds.right=Math.max(bounds.right,x);bounds.top=Math.min(bounds.top,y);bounds.bottom=Math.max(bounds.bottom,y);}}
        return {changedPixels,totalPixels:canvas.width*canvas.height,maxChannelDelta,meanChannelDelta:sumDelta/a.length,bounds:changedPixels?bounds:undefined};
      },{before:before.toString('base64'),after:after.toString('base64')});
      comparisons.push({page:index+1,exact:pixels.changedPixels===0,byteExact,...pixels,baselineHash:hash(before),sdkHash:hash(after),baseline:prefix+'-baseline.png',sdk:prefix+'-sdk.png'});
    }
    results.push({kind,name,...result,pages:count,exact:comparisons.every(item=>item.exact),comparisons});
    console.log(JSON.stringify({name,pages:count,exact:results.at(-1).exact}));
  }
  await page.evaluate(()=>window.regression.close());
} finally {
  await writeFile(resolve(output,'results.json'),JSON.stringify({baselineRevision,consumer,harness,width:1100,deviceScaleFactor:1,comparison:'Chrome PNG and decoded RGBA comparison; same parsed model and host width; no tolerance',results,browserErrors,failedRequests},null,2));
  await browser.close();
  await new Promise(done=>server.close(done));
}
assert.deepEqual(browserErrors,[],'Browser errors');
assert.deepEqual(failedRequests,[],'Failed requests');
assert.equal(results.length,samples.length,'Incomplete visual regression');
assert.ok(results.every(result=>result.exact),'Visual differences found: inspect output/playwright/visual-regression/results.json');
console.log(`PASS: ${results.reduce((sum,result)=>sum+result.pages,0)} pages/sheets across ${results.length} samples match the pre-SDK renderer exactly.`);
