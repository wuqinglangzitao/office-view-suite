import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, extname, sep } from 'node:path';

// The geometry probe deliberately uses an independently installed, built SDK.
// Only the standalone layout module is injected so its scroll effects can be
// compared with the identical pre-watermark renderer in the same browser.
const require = createRequire(new URL('../packages/viewer/package.json', import.meta.url));
const ts = require('typescript');
const source = await readFile(new URL('../packages/viewer/src/watermark-layout.ts', import.meta.url), 'utf8');
const moduleSource = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { consumer } = JSON.parse(await readFile(new URL('../output/packed-consumer.json', import.meta.url)));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(consumer, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  if (path === '/watermark-layout.js') { res.writeHead(200, { 'Content-Type': 'text/javascript' }).end(moduleSource); return; }
  if (!path.startsWith('/office-view/')) { res.writeHead(404).end(); return; }
  const file = resolve(root, path.slice('/office-view/'.length) || 'index.html');
  if (file !== root && !file.startsWith(root + sep)) { res.writeHead(403).end(); return; }
  try { res.writeHead(200, { 'Content-Type': mime[extname(file)] || 'application/octet-stream' }).end(await readFile(file)); }
  catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const output = new URL('../output/playwright/watermark-layout/', import.meta.url);
await mkdir(output, { recursive: true });
const report = { browser: browser.version(), scenarios: [], errors: [] };
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.on('pageerror', error => report.errors.push(error.message));
try {
  await page.goto(`http://127.0.0.1:${server.address().port}/office-view/`);
  await page.waitForFunction(() => !!window.officeFixture);
  await page.evaluate(async () => { window.watermarkLayoutModule = await import('/watermark-layout.js'); });
  for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
    const results = await page.evaluate(async kind => {
      const host = document.createElement('div');
      host.style.cssText = 'width:640px;min-height:0;overflow:auto;';
      document.body.prepend(host);
      const viewer = window.officeFixture.create(host);
      const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const failures = [];
      const results = [];
      const metric = node => [node.clientWidth, node.clientHeight, node.scrollWidth, node.scrollHeight, node.scrollLeft, node.scrollTop];
      let layout;
      try {
        await viewer.open(await window.officeFixture.sample(kind));
        await settle();
        await settle();
        const viewport = host.querySelector('.office-viewer-host');
        const surface = host.querySelector('.office-viewer-surface');
        const content = surface.shadowRoot.querySelector('.office-viewer-content');
        layout = window.watermarkLayoutModule.createWatermarkLayout(viewport, surface, content);
        const canvas = document.createElement('canvas');
        canvas.width = 240; canvas.height = 120;
        const context = canvas.getContext('2d');
        context.translate(120, 60); context.rotate(-25 * Math.PI / 180);
        context.font = '18px sans-serif'; context.fillStyle = 'rgba(0,0,0,0.2)';
        context.textAlign = 'center'; context.fillText('内部资料 · 水印验证', 0, 0);
        const tile = { url: canvas.toDataURL(), width: 240, height: 120 };
        const check = (name, before, after) => {
          for (const node of Object.keys(before)) {
            for (let i = 0; i < before[node].length; i++) {
              if (Math.abs(before[node][i] - after[node][i]) > 1) failures.push({ name, node, index: i, before: before[node][i], after: after[node][i] });
            }
          }
        };
        for (const bounded of [false, true]) {
          host.style.height = bounded ? '400px' : 'auto';
          viewer.resize();
          for (const zoom of [0.1, 1, 4, 0.1]) {
            viewer.setZoom(zoom);
            await settle();
            await settle();
            viewport.scrollLeft = 137;
            viewport.scrollTop = 17;
            host.scrollTop = 29;
            await settle();
            const snapshot = () => ({ host: metric(host), viewport: metric(viewport), surface: metric(surface), content: metric(content) });
            const before = snapshot();
            layout.show(tile);
            await settle();
            const enabled = snapshot();
            const bounds = window.watermarkLayoutModule.measureWatermarkBounds(viewport, surface, content);
            const overlay = viewport.querySelector('.office-viewer-watermark');
            const overlayRect = overlay.getBoundingClientRect();
            if (Math.abs(overlayRect.width - bounds.width) > 1 || Math.abs(overlayRect.height - bounds.height) > 1) throw new Error(`Coverage mismatch ${kind}/${zoom}`);
            if (getComputedStyle(overlay).pointerEvents !== 'none' || overlay.getAttribute('aria-hidden') !== 'true') throw new Error('Interactive overlay');
            check(`${kind}/${bounded}/${zoom}/enable`, before, enabled);
            layout.hide();
            await settle();
            const disabled = snapshot();
            check(`${kind}/${bounded}/${zoom}/disable`, before, disabled);
            results.push({ kind, bounded, zoom, before, enabled, disabled, bounds });
          }
        }
        layout.show(tile);
        await settle();
        host.style.display = 'none';
        await settle();
        if (viewport.querySelector('.office-viewer-watermark').style.display !== 'none') throw new Error('Hidden host left an active overlay');
        host.style.display = 'block';
        await settle();
        if (viewport.querySelector('.office-viewer-watermark').style.display !== 'block') throw new Error('Overlay did not recover after show');
        // A renderer can add pagination after open(), without changing the
        // bounded external host. No explicit geometry request should be needed.
        if (kind === 'word') {
          viewer.setZoom(1);
          await settle();
          const beforeHeight = parseFloat(viewport.querySelector('.office-viewer-watermark').style.height);
          const extra = document.createElement('div'); extra.style.height = '1600px';
          content.appendChild(extra);
          await settle();
          const afterHeight = parseFloat(viewport.querySelector('.office-viewer-watermark').style.height);
          if (afterHeight < beforeHeight + 1599) throw new Error('Progressive pagination was not covered');
          extra.remove();
          await settle();
          const recovered = parseFloat(viewport.querySelector('.office-viewer-watermark').style.height);
          if (Math.abs(recovered - beforeHeight) > 1) throw new Error('Overlay prevented scroll range shrinking');
          results.push({ kind, growth: { beforeHeight, afterHeight, recovered } });
        }
        layout.hide();
        if (viewport.style.position || viewport.style.isolation || surface.style.isolation) throw new Error('Local stacking styles were not restored');
        if (failures.length) throw new Error(JSON.stringify(failures));
        return results;
      } finally {
        layout?.destroy();
        viewer.destroy();
        host.remove();
      }
    }, kind);
    report.scenarios.push(...results);
    await page.evaluate(async kind => {
      const viewer = window.officeFixture.viewer;
      viewer.setZoom(1);
      await viewer.open(await window.officeFixture.sample(kind));
      const viewport = document.querySelector('#viewer .office-viewer-host');
      const surface = viewport.querySelector('.office-viewer-surface');
      window.layoutScreenshotProbe = window.watermarkLayoutModule.createWatermarkLayout(viewport, surface,
        surface.shadowRoot.querySelector('.office-viewer-content'));
      const canvas = document.createElement('canvas');
      canvas.width = 240; canvas.height = 120;
      const context = canvas.getContext('2d');
      context.translate(120, 60); context.rotate(-25 * Math.PI / 180);
      context.font = '18px sans-serif'; context.fillStyle = 'rgba(0,0,0,0.2)';
      context.textAlign = 'center'; context.fillText('内部资料 · 水印验证', 0, 0);
      window.layoutScreenshotProbe.show({ url: canvas.toDataURL(), width: 240, height: 120 });
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    }, kind);
    await page.locator('#viewer').screenshot({ path: new URL(`${kind}.png`, output).pathname });
    await page.evaluate(() => { window.layoutScreenshotProbe.destroy(); window.officeFixture.viewer.clear(); });
    console.log(`Passed watermark geometry ${kind}: ${results.length} scenarios`);
  }
  assert.deepEqual(report.errors, []);
} finally {
  await writeFile(new URL('results.json', output), JSON.stringify(report, null, 2));
  await browser.close();
  await new Promise(r => server.close(r));
}
