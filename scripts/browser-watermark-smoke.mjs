import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, extname, sep } from 'node:path';

const { consumer } = JSON.parse(await readFile(new URL('../output/packed-consumer.json', import.meta.url)));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(consumer, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://localhost');
  const path = decodeURIComponent(url.pathname);
  if (!path.startsWith('/office-view/')) { res.writeHead(404).end(); return; }
  const file = resolve(root, path.slice('/office-view/'.length) || 'index.html');
  if (file !== root && !file.startsWith(root + sep)) { res.writeHead(403).end(); return; }
  try {
    const headers = { 'Content-Type': mime[extname(file)] || 'application/octet-stream' };
    if (url.searchParams.has('csp')) headers['Content-Security-Policy'] = "img-src 'self'";
    res.writeHead(200, headers).end(await readFile(file));
  } catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}/office-view/`;
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const output = new URL('../output/playwright/watermark/', import.meta.url);
await mkdir(output, { recursive: true });
const report = { browser: browser.version(), consumer, results: [], errors: [] };
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.on('pageerror', error => report.errors.push(error.message));
await page.addInitScript(() => {
  const OriginalWorker = window.Worker;
  window.watermarkWorkerCount = 0;
  window.Worker = new Proxy(OriginalWorker, {
    construct(Target, args) { window.watermarkWorkerCount++; return new Target(...args); },
  });
});

try {
  await page.goto(base);
  await page.waitForFunction(() => !!window.officeFixture);
  await page.evaluate(() => {
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    const frames = async (count = 3) => {
      for (let i = 0; i < count; i++) await new Promise(resolve => requestAnimationFrame(resolve));
    };
    const overlay = host => host.querySelector('.office-viewer-watermark');
    const pattern = host => overlay(host)?.shadowRoot.firstElementChild;
    const pixels = async host => {
      const background = pattern(host).style.backgroundImage;
      const url = background.slice(5, -2);
      const image = new Image(); image.src = url; await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
      const ctx = canvas.getContext('2d'); ctx.drawImage(image, 0, 0);
      const rgba = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let painted = 0;
      for (let i = 3; i < rgba.length; i += 4) if (rgba[i] > 0) painted++;
      assert(painted > 0, 'Watermark PNG has no visible pixels');
      const result = { width: canvas.width, height: canvas.height, painted, backgroundSize: pattern(host).style.backgroundSize };
      canvas.width = canvas.height = 0;
      return result;
    };
    const metrics = host => {
      const viewport = host.querySelector('.office-viewer-host');
      const surface = host.querySelector('.office-viewer-surface');
      return [host, viewport, surface].map(node => [node.clientWidth, node.clientHeight, node.scrollWidth,
        node.scrollHeight, node.scrollTop, node.scrollLeft]);
    };
    const equalGeometry = (before, after) => assert(before.every((node, n) => node.every((value, i) =>
      Math.abs(value - after[n][i]) <= 1)), `Watermark changed geometry: ${JSON.stringify({ before, after })}`);
    window.watermarkTest = { assert, frames, overlay, pattern, pixels, metrics, equalGeometry };
  });

  for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
    const result = await page.evaluate(async kind => {
      const t = window.watermarkTest;
      const viewer = window.officeFixture.viewer;
      const host = document.querySelector('#viewer');
      host.style.height = '500px'; host.style.overflow = 'auto';
      const source = await window.officeFixture.sample(kind);
      await viewer.setWatermark(null);
      await viewer.open(source);
      await t.frames(5);
      const surface = host.querySelector('.office-viewer-surface');
      const selector = { word: '.page', excel: '.workbook', ppt: '.slide', pdf: 'canvas' }[kind];
      const documentNode = surface.shadowRoot.querySelector(selector);
      const workers = window.watermarkWorkerCount;
      const zoomResults = [];
      for (const scale of [0.1, 1, 4, 0.1]) {
        await viewer.setWatermark(null);
        viewer.setZoom(scale); await t.frames(4);
        host.scrollTop = 15;
        surface.parentElement.scrollLeft = 29;
        await t.frames();
        const before = t.metrics(host);
        const content = ['内部资料 · 张三（00123）', '2026-10-22'];
        const gap = [120, 80];
        await viewer.setWatermark({ content, gap });
        content[0] = 'MUTATED'; gap[0] = 512;
        await t.frames();
        t.equalGeometry(before, t.metrics(host));
        const config = viewer.getWatermark();
        t.assert(config.content[0].includes('张三') && config.gap[0] === 120, 'Caller configuration leaked into viewer');
        config.content[0] = 'MUTATED'; config.gap[0] = 0;
        t.assert(viewer.getWatermark().content[0].includes('张三') && viewer.getWatermark().gap[0] === 120, 'Getter leaked configuration references');
        const pixels = await t.pixels(host);
        zoomResults.push({ scale, pixels });
        t.assert(t.pattern(host).style.backgroundSize === zoomResults[0].pixels.backgroundSize, 'Document zoom changed watermark tile density');
        t.assert(surface.shadowRoot.querySelector(selector) === documentNode, 'Watermark update replaced document DOM');
        t.assert(window.watermarkWorkerCount === workers, 'Watermark update created another Worker');
      }
      viewer.setZoom(1); await t.frames();
      const updated = { content: ['内部资料 · 李四（00456）'], color: '#f00', opacity: 0.2 };
      await viewer.setWatermark(updated);
      t.assert(viewer.getWatermark().color === '#ff0000', 'Color normalization failed');
      t.assert(viewer.getWatermark().gap[0] === 120 && viewer.getWatermark().fontSize === 14, 'Full replacement did not restore defaults');
      const invalid = await viewer.setWatermark({ content: '' }).then(() => 'BAD', error => error.code);
      t.assert(invalid === 'INVALID_WATERMARK' && viewer.getWatermark().content[0].includes('李四'), 'Invalid update replaced applied watermark');
      const selected = surface.shadowRoot.querySelector('.sheet-tabs button');
      if (selected) {
        const box = selected.getBoundingClientRect();
        const hit = surface.shadowRoot.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
        t.assert(hit === selected || selected.contains(hit), 'Watermark intercepted Excel tab hit test');
      }
      viewer.clear();
      t.assert(!t.overlay(host) && viewer.getWatermark().content[0].includes('李四'), 'clear() lost persistent configuration or kept overlay');
      await viewer.open(source); await t.frames();
      t.assert(!!t.overlay(host) && viewer.getWatermark().content[0].includes('李四'), 'reopen() did not restore watermark');
      return { kind, zoomResults, invalid, workers };
    }, kind);
    if (kind === 'excel') await page.locator('#viewer').getByRole('tab').first().click();
    await page.locator('#viewer').screenshot({ path: new URL(`${kind}.png`, output).pathname });
    report.results.push({ test: `packed-watermark-${kind}`, ...result });
    console.log(`Passed packed watermark ${kind}`);
  }

  report.results.push(await page.evaluate(async () => {
    const t = window.watermarkTest;
    const viewer = window.officeFixture.viewer;
    const before = viewer.getWatermark();
    const invalidFonts = ['initial', 'inherit', 'unset', 'revert', 'revert-layer', 'var(--font)', '\\69 nitial'];
    for (const fontFamily of invalidFonts) {
      const code = await viewer.setWatermark({ content: '字体校验', fontSize: 72, fontFamily })
        .then(() => 'BAD', error => error.code);
      t.assert(code === 'INVALID_WATERMARK', `Canvas-incompatible font accepted: ${fontFamily}`);
    }
    t.assert(JSON.stringify(viewer.getWatermark()) === JSON.stringify(before), 'Invalid font changed applied configuration');
    return { test: 'canvas-font-validation', invalidFonts: invalidFonts.length };
  }));

  report.results.push(await page.evaluate(async () => {
    const t = window.watermarkTest;
    const viewer = window.officeFixture.viewer;
    const host = document.querySelector('#viewer');
    await viewer.open(await window.officeFixture.sample('ppt'));
    await viewer.setWatermark({ content: '保留旧水印' });
    const old = t.pattern(host).style.backgroundImage;
    const events = [];
    const stop = viewer.on(event => events.push(event));
    const original = HTMLCanvasElement.prototype.toDataURL;
    let caught;
    try {
      HTMLCanvasElement.prototype.toDataURL = () => { throw new Error('simulated encoding failure'); };
      caught = await viewer.setWatermark({ content: '更新失败' }).then(() => null, error => error);
    } finally { HTMLCanvasElement.prototype.toDataURL = original; }
    t.assert(caught?.code === 'WATERMARK_FAILED', 'Encoding failure did not reject setter');
    t.assert(viewer.getWatermark().content[0] === '保留旧水印' && t.pattern(host).style.backgroundImage === old, 'Encoding failure lost old watermark');
    t.assert(events.filter(event => event.type === 'watermarkerror').length === 1, 'Encoding failure emitted incorrect event count');
    t.assert(events.find(event => event.type === 'watermarkerror').error === caught, 'Promise and watermark event lost error identity');
    t.assert(!events.some(event => event.type === 'error' || event.type === 'statechange'), 'Runtime failure changed document state');
    stop();
    return { test: 'runtime-encoding-failure-preserves-ready', code: caught.code };
  }));

  report.results.push(await page.evaluate(async () => {
    const t = window.watermarkTest;
    const host = document.createElement('div'); host.style.width = '640px'; document.body.appendChild(host);
    const source = await window.officeFixture.sample('word');
    const context = HTMLCanvasElement.prototype.getContext;
    let viewer;
    const events = [];
    try {
      HTMLCanvasElement.prototype.getContext = () => null;
      viewer = window.officeFixture.create(host, { watermark: { content: '必须带水印' } });
      viewer.on(event => events.push(event));
      const error = await viewer.open(source).then(() => null, error => error);
      t.assert(error?.code === 'WATERMARK_FAILED', 'Initial Canvas failure did not block open');
      t.assert(!host.querySelector('.office-viewer-surface').shadowRoot.querySelector('.page'), 'Initial failure exposed unmarked document');
      const watermarkError = events.find(event => event.type === 'watermarkerror');
      const openError = events.find(event => event.type === 'error');
      t.assert(watermarkError?.error === error && openError?.error === error, 'Initial/open errors did not share identity');
      viewer.clear();
      const again = await viewer.open(source).then(() => null, error => error);
      t.assert(again === error, 'clear() silently removed initial failure');
      HTMLCanvasElement.prototype.getContext = context;
      await viewer.setWatermark(null);
      await viewer.open(source);
      t.assert(viewer.getWatermark() === null, 'Explicit null did not recover failed initial watermark');
      return { test: 'initial-canvas-failure-blocks-and-null-recovers', code: error.code };
    } finally { HTMLCanvasElement.prototype.getContext = context; viewer?.destroy(); host.remove(); }
  }));

  report.results.push(await page.evaluate(async () => {
    const t = window.watermarkTest;
    const viewer = window.officeFixture.viewer;
    const host = document.querySelector('#viewer');
    await viewer.setWatermark({ content: ['字体与 DPR', '刷新水印'] });
    const before = await t.pixels(host);
    const original = HTMLCanvasElement.prototype.toDataURL;
    let draws = 0;
    HTMLCanvasElement.prototype.toDataURL = function(...args) { draws++; return original.apply(this, args); };
    try {
      document.fonts.dispatchEvent(new Event('loadingdone'));
      for (let i = 0; i < 30 && !draws; i++) await t.frames();
      t.assert(draws > 0, 'Font completion did not redraw tile');
      await t.frames(5);
      const afterFont = await t.pixels(host);
      t.assert(afterFont.backgroundSize === before.backgroundSize, 'Unchanged font changed logical tile size');
      const fontDraws = draws;
      const dpr = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');
      const nativeDpr = window.devicePixelRatio;
      try {
        Object.defineProperty(window, 'devicePixelRatio', { configurable: true, get: () => nativeDpr === 2 ? 3 : 2 });
        window.dispatchEvent(new Event('resize'));
        for (let i = 0; i < 30 && draws === fontDraws; i++) await t.frames();
        await t.frames(5);
        const afterDpr = await t.pixels(host);
        t.assert(draws > fontDraws && afterDpr.width > before.width, 'DPR update did not increase bitmap resolution');
        t.assert(afterDpr.backgroundSize === before.backgroundSize, 'DPR changed logical density');
        return { test: 'font-and-dpr-refresh', before, afterFont, afterDpr, draws };
      } finally {
        if (dpr) Object.defineProperty(window, 'devicePixelRatio', dpr);
        else delete window.devicePixelRatio;
        window.dispatchEvent(new Event('resize'));
      }
    } finally { HTMLCanvasElement.prototype.toDataURL = original; }
  }));

  report.results.push(await page.evaluate(async () => {
    const t = window.watermarkTest;
    const host = document.createElement('div'); document.body.appendChild(host);
    const initial = { content: ['初始化水印', '保密'] };
    const viewer = window.officeFixture.create(host, { watermark: initial });
    try {
      t.assert(viewer.getWatermark() === null, 'Initial configuration committed synchronously');
      await viewer.open(await window.officeFixture.sample('ppt'));
      t.assert(viewer.getWatermark().content[0] === '初始化水印' && !!t.overlay(host), 'First open lacked initial watermark');
      viewer.destroy();
      t.assert(viewer.getWatermark() === null && !host.querySelector('.office-viewer-host'), 'destroy() retained markup/config');
      const code = await viewer.setWatermark(initial).then(() => 'BAD', error => error.code);
      t.assert(code === 'DESTROYED', 'Destroyed setter did not reject');
      return { test: 'initial-and-destroy', code };
    } finally { viewer.destroy(); host.remove(); }
  }));

  const cspPage = await browser.newPage();
  try {
    await cspPage.goto(`${base}?csp=1`);
    await cspPage.waitForFunction(() => !!window.officeFixture);
    const csp = await cspPage.evaluate(async () => {
      const host = document.createElement('div'); document.body.appendChild(host);
      const viewer = window.officeFixture.create(host, { watermark: { content: 'CSP 检查' } });
      const events = [];
      viewer.on(event => events.push(event));
      try {
        const error = await viewer.open(await window.officeFixture.sample('ppt')).then(() => null, error => error);
        if (error?.code !== 'WATERMARK_FAILED' || events.find(event => event.type === 'watermarkerror')?.error !== error
          || host.querySelector('.office-viewer-watermark')) throw new Error('CSP did not block watermark explicitly');
        return { test: 'csp-blocks-data-images', code: error.code };
      } finally { viewer.destroy(); host.remove(); }
    });
    report.results.push(csp);
  } finally { await cspPage.close(); }
  assert.deepEqual(report.errors, [], 'Unhandled browser errors');
  console.log(JSON.stringify({ passed: report.results.length, results: report.results }, null, 2));
} finally {
  await writeFile(new URL('results.json', output), JSON.stringify(report, null, 2));
  await browser.close();
  await new Promise(r => server.close(r));
}
