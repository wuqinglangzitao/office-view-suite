import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, extname, sep } from 'node:path';
import assert from 'node:assert/strict';

const { consumer } = JSON.parse(await readFile(new URL('../output/packed-consumer.json', import.meta.url)));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(consumer, 'dist');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  if (!path.startsWith('/office-view/')) { res.writeHead(404).end(); return; }
  const relative = path.slice('/office-view/'.length);
  const fault = relative.match(/^(fault-mime|fault-cors)\//)?.[1];
  const file = resolve(root, (fault ? relative.slice(fault.length + 1) : relative) || 'index.html');
  if (file !== root && !file.startsWith(root + sep)) { res.writeHead(403).end(); return; }
  try {
    const body = await readFile(file);
    const headers = { 'Content-Type': fault === 'fault-mime' && extname(file) === '.js' ? 'text/plain' : mime[extname(file)] || 'application/octet-stream' };
    if (fault !== 'fault-cors') headers['Access-Control-Allow-Origin'] = '*';
    res.writeHead(200, headers);
    res.end(body);
  } catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '0.0.0.0', r));
const port = server.address().port;
const base = `http://127.0.0.1:${port}/office-view/`;
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [], failures = [], requests = [], results = [];
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.on('pageerror', error => errors.push(error.message));
page.on('requestfailed', req => failures.push({ url: req.url(), error: req.failure()?.errorText }));
page.on('request', req => requests.push(req.url()));
const output = new URL('../output/playwright/', import.meta.url);
await mkdir(output, { recursive: true });
try {
  await page.goto(base);
  await page.waitForFunction(() => !!window.officeFixture);
  for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
    const result = await page.evaluate(async kind => {
      const f = window.officeFixture;
      const source = await f.sample(kind);
      const length = source.bytes.byteLength;
      const result = await f.viewer.open(source);
      if (source.bytes.byteLength !== length) throw new Error('Caller buffer detached');
      return result;
    }, kind);
    assert.ok(Object.keys(result.stats).length > 0);
    await page.waitForFunction(() => {
      const host = document.querySelector('#viewer .office-viewer-surface');
      return host?.shadowRoot?.textContent.length > 100;
    });
    if (kind === 'pdf') await page.waitForFunction(() => document.querySelector('#viewer .office-viewer-surface')?.shadowRoot?.querySelector('canvas')?.width > 300);
    await page.screenshot({ path: new URL(`${kind}.png`, output).pathname });
    results.push({ test: `packed-vite-${kind}`, result });
    const zoom = await page.evaluate(async kind => {
      const viewer = window.officeFixture.viewer;
      const surface = document.querySelector('#viewer .office-viewer-surface');
      const selector = { word: '.page', excel: '.cell', ppt: '.slide', pdf: 'canvas' }[kind];
      const contentWidth = () => surface.shadowRoot.querySelector(selector).getBoundingClientRect().width;
      const originalWidth = contentWidth();
      const capabilities = viewer.getCapabilities();
      if (!capabilities.zoom || !capabilities.fitWidth) throw new Error('Missing preview capabilities');
      viewer.setZoom(1.5);
      viewer.resize();
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const fixed = viewer.getZoom();
      if (fixed.mode !== 'fixed' || fixed.scale !== 1.5) throw new Error('Fixed zoom did not persist');
      const scaledWidth = contentWidth();
      if (Math.abs(scaledWidth / originalWidth - 1.5) > 0.05) throw new Error(`Zoom geometry mismatch: ${originalWidth} -> ${scaledWidth}`);
      viewer.setZoom('fit-width');
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const fit = viewer.getZoom();
      if (fit.mode !== 'fit-width' || !Number.isFinite(fit.scale) || fit.scale <= 0) throw new Error('Invalid fit width scale');
      const fitted = surface.shadowRoot.querySelector(kind === 'excel' ? '.grid' : selector).getBoundingClientRect().width;
      const available = surface.parentElement.clientWidth;
      if (fit.scale > 0.1 && fitted > available + 2) throw new Error(`Fit width overflows: ${fitted} > ${available}`);
      viewer.setZoom(1);
      return { fixed, fit, originalWidth, scaledWidth, fitted, available };
    }, kind);
    results.push({ test: `preview-zoom-${kind}`, ...zoom });
  }
  // Exercise real IntersectionObserver clipping, bitmap eviction and repainting.
  await page.evaluate(async () => {
    const host = document.querySelector('#viewer');
    host.style.height = '400px';
    host.style.minHeight = '0';
    host.style.overflow = 'auto';
    window.scrollTo(0, 0);
    await window.officeFixture.viewer.open(await window.officeFixture.sample('pdf'));
  });
  await page.waitForFunction(() => {
    const canvases = document.querySelector('#viewer .office-viewer-surface').shadowRoot.querySelectorAll('canvas');
    return canvases.length === 2 && canvases[0].width > 300 && canvases[1].width === 0;
  });
  const firstPage = await page.evaluate(() => {
    const host = document.querySelector('#viewer');
    const canvas = host.querySelector('.office-viewer-surface').shadowRoot.querySelector('canvas');
    const snapshot = { pixels: canvas.toDataURL(), height: host.scrollHeight, cssWidth: canvas.style.width, cssHeight: canvas.style.height };
    host.scrollTop = host.scrollHeight;
    return snapshot;
  });
  await page.waitForFunction(() => {
    const canvases = document.querySelector('#viewer .office-viewer-surface').shadowRoot.querySelectorAll('canvas');
    return canvases[0].width === 0 && canvases[0].height === 0 && canvases[1].width > 300;
  });
  await page.evaluate(first => {
    const host = document.querySelector('#viewer');
    const canvas = host.querySelector('.office-viewer-surface').shadowRoot.querySelector('canvas');
    if (host.scrollHeight !== first.height || canvas.style.width !== first.cssWidth || canvas.style.height !== first.cssHeight) {
      throw new Error('PDF eviction changed scroll layout');
    }
    host.scrollTop = 0;
  }, { height: firstPage.height, cssWidth: firstPage.cssWidth, cssHeight: firstPage.cssHeight });
  await page.waitForFunction(() => {
    const canvases = document.querySelector('#viewer .office-viewer-surface').shadowRoot.querySelectorAll('canvas');
    return canvases[0].width > 300 && canvases[1].width === 0;
  });
  const cache = await page.evaluate(firstPixels => {
    const host = document.querySelector('#viewer');
    const canvases = [...host.querySelector('.office-viewer-surface').shadowRoot.querySelectorAll('canvas')];
    if (canvases[0].toDataURL() !== firstPixels) throw new Error('PDF repaint changed page pixels');
    const retained = canvases.filter(canvas => canvas.width > 0).length;
    window.officeFixture.viewer.clear();
    if (canvases.some(canvas => canvas.width || canvas.height)) throw new Error('PDF clear retained pixel storage');
    host.style.removeProperty('height');
    host.style.removeProperty('min-height');
    host.style.removeProperty('overflow');
    return { pages: canvases.length, retained, pixelsMatch: true };
  }, firstPage.pixels);
  results.push({ test: 'pdf-bitmap-eviction-and-repaint', ...cache });
  results.push(await page.evaluate(async () => {
    const f = window.officeFixture;
    const word = await f.sample('word'), ppt = await f.sample('ppt');
    const both = await Promise.all([f.viewer.open(word), f.second.open(ppt)]);
    const aborted = new AbortController(); aborted.abort();
    const code = await f.viewer.open({ ...word, signal: aborted.signal }).then(() => 'BAD', e => e.code);
    if (code !== 'ABORTED') throw new Error(`pre-abort: ${code}`);
    const pending = f.viewer.open(word).then(() => 'resolved', e => e.code);
    const replacement = await f.viewer.open(ppt);
    if (await pending !== 'ABORTED' || replacement.format !== 'pptx') throw new Error('supersession failed');
    f.viewer.clear();
    if (document.querySelector('#viewer .office-viewer-surface')?.shadowRoot?.querySelector('.slide')) throw new Error('clear failed');
    const liveAbort = new AbortController();
    const canceled = f.viewer.open({ ...word, signal: liveAbort.signal }).then(() => 'BAD', e => e.code);
    liveAbort.abort();
    if (await canceled !== 'ABORTED') throw new Error('live abort failed');
    const host = document.createElement('div'); document.body.append(host);
    const marker = document.createElement('span'); marker.textContent = 'host-owned'; host.append(marker);
    const doomed = f.create(host);
    const pendingDestroy = doomed.open(word).then(() => 'BAD', e => e.code); doomed.destroy();
    if (await pendingDestroy !== 'DESTROYED') throw new Error('destroy failed');
    if (!host.contains(marker)) throw new Error('host DOM removed');
    host.remove();
    return { test: 'multiple-instances-and-lifecycle', formats: both.map(r => r.format) };
  }));
  // Asset overrides use a different origin; the host controls the Worker bootstrap.
  for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
    const result = await page.evaluate(async ({kind, port}) => {
      const host = document.createElement('div'); document.body.append(host);
      const urls = [];
      const viewer = window.officeFixture.create(host, {
        assets: { baseUrl: `http://localhost:${port}/office-view/office-assets` },
        workerFactory(url, options) {
          const source = options?.type === 'module' ? `import ${JSON.stringify(url.href)}` : `importScripts(${JSON.stringify(url.href)})`;
          const blob = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
          urls.push(blob);
          return new Worker(blob, options);
        },
      });
      try { return await viewer.open(await window.officeFixture.sample(kind)); }
      finally { viewer.destroy(); host.remove(); urls.forEach(url => URL.revokeObjectURL(url)); }
    }, {kind, port});
    results.push({test: `cross-origin-assets-${kind}`, format: result.format});
  }
  await page.goto(`${base}native.html`);
  await page.waitForFunction(() => !!window.nativeViewer);
  for (const name of ['styles-word.docx', 'styles-excel.xlsx', 'styles-ppt.pptx', 'handmade-mixed.pdf']) {
    const result = await page.evaluate(async name => {
      const bytes = await (await fetch(`./samples/${name}`)).arrayBuffer();
      return await window.nativeViewer.open({ bytes, name });
    }, name);
    results.push({test: `native-esm-${name}`, format:result.format});
  }
  assert.deepEqual(errors, [], 'Unhandled browser errors');
  assert.deepEqual(failures, [], 'Failed asset requests');
  assert.ok(requests.some(url => new URL(url).pathname.endsWith('.wasm')), 'No WASM requests');
  assert.ok(new Set(requests.filter(url => url.startsWith(base) && /\/assets\/worker-.*\.js$/.test(new URL(url).pathname))).size >= 4, 'All four workers must stay separate assets');
  // Expected deployment failures use separate pages so normal-load errors remain strict.
  for (const fault of ['fault-mime', 'fault-cors']) {
    const faultPage = await browser.newPage();
    try {
      await faultPage.goto(base);
      await faultPage.waitForFunction(() => !!window.officeFixture);
      const code = await faultPage.evaluate(async ({ fault, port }) => {
        const host = document.createElement('div'); document.body.append(host);
        const blobs = [];
        const viewer = window.officeFixture.create(host, {
          assets: { baseUrl: `http://localhost:${port}/office-view/${fault}/office-assets/` },
          workerFactory(url, options) {
            const blob = URL.createObjectURL(new Blob([`import ${JSON.stringify(url.href)}`], { type: 'text/javascript' }));
            blobs.push(blob);
            return new Worker(blob, options);
          },
        });
        let timer;
        try {
          const source = await window.officeFixture.sample('excel');
          return await Promise.race([
            viewer.open(source).then(() => 'UNEXPECTED_SUCCESS', error => error.code),
            new Promise(resolve => { timer = setTimeout(() => resolve('TIMEOUT'), 10000); }),
          ]);
        } finally {
          clearTimeout(timer);
          viewer.destroy(); host.remove(); blobs.forEach(url => URL.revokeObjectURL(url));
        }
      }, { fault, port });
      assert.equal(code, 'ASSET_LOAD_FAILED', `${fault} must report an asset failure`);
      results.push({ test: fault, code });
    } finally { await faultPage.close(); }
  }
  await writeFile(new URL('sdk-results.json', output), JSON.stringify({ results, errors, failures, assetRequests: requests.filter(url => /wasm|worker|pdfium/.test(url)) }, null, 2));
  console.log(JSON.stringify({ passed: results.length, results, errors, failures }, null, 2));
} finally {
  await browser.close();
  await new Promise(r => server.close(r));
}
