import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const { consumer: packed } = JSON.parse(await readFile(resolve(root, 'output/packed-consumer.json')));
const consumer = await realpath(await mkdtemp(resolve(tmpdir(), 'office-vue2-consumer-')));
const fixture = resolve(root, 'fixtures/vue2-vite');
const names = ['viewer-core', 'viewer-word', 'viewer-excel', 'viewer-ppt', 'viewer-pdf', 'viewer'];
const metadata = JSON.parse(await readFile(resolve(fixture, 'package.json')));
const dependencies = Object.fromEntries(names.map(name => [
  `@office-view-suite/${name}`, `file:./tarballs/office-view-suite-${name}-0.1.0.tgz`,
]));
const run = (args) => execFileSync('pnpm', args, { cwd: consumer, stdio: 'inherit' });
for (const file of ['index.html', 'vite.config.js', 'src']) {
  await cp(resolve(fixture, file), resolve(consumer, file), { recursive: true });
}
await cp(resolve(packed, 'tarballs'), resolve(consumer, 'tarballs'), { recursive: true });
await cp(resolve(packed, 'public/samples'), resolve(consumer, 'public/samples'), { recursive: true });
await writeFile(resolve(consumer, 'package.json'), JSON.stringify({
  ...metadata,
  scripts: { dev: 'vite', build: 'vite build' },
  dependencies: { ...metadata.dependencies, ...dependencies },
  pnpm: { overrides: dependencies },
}, null, 2));
run(['install', '--prefer-offline', '--ignore-scripts']);
for (const name of names) {
  const installed = resolve(consumer, `node_modules/@office-view-suite/${name}`);
  assert.ok((await realpath(installed)).startsWith(consumer + sep), `SDK must be installed independently: ${name}`);
  const dist = resolve(installed, 'dist');
  const licenses = resolve(consumer, 'public/office-licenses', name);
  await mkdir(licenses, { recursive: true });
  await cp(resolve(dist, 'LICENSE'), resolve(licenses, 'LICENSE'));
  if (name === 'viewer' || name === 'viewer-core') continue;
  await cp(resolve(dist, 'assets'), resolve(consumer, 'public/office-assets'), { recursive: true });
  await cp(resolve(dist, 'THIRD_PARTY_NOTICES.md'), resolve(licenses, 'THIRD_PARTY_NOTICES.md'));
  await cp(resolve(dist, 'licenses'), resolve(licenses, 'licenses'), { recursive: true });
}
run(['build']);

const vite = await import(pathToFileURL(resolve(consumer, 'node_modules/vite/dist/node/index.js')));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const output = resolve(root, 'output/playwright/vue2');
await mkdir(output, { recursive: true });
const report = { consumer, browser: browser.version(), versions: {}, suites: [] };
for (const name of ['vue', 'vite', '@vitejs/plugin-vue2']) {
  report.versions[name] = JSON.parse(await readFile(resolve(consumer, `node_modules/${name}/package.json`))).version;
}

async function verify(base, mode, assetMode) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const suite = { mode, assetMode, passed: false, results: [], errors: [], failures: [], httpErrors: [], assetRequests: [] };
  report.suites.push(suite);
  page.on('pageerror', error => suite.errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') suite.errors.push(message.text()); });
  page.on('requestfailed', request => suite.failures.push({ url: request.url(), error: request.failure()?.errorText }));
  page.on('response', response => { if (response.status() >= 400) suite.httpErrors.push({ url: response.url(), status: response.status() }); });
  page.on('request', request => {
    if (/worker|\.wasm|pdfium/.test(request.url())) suite.assetRequests.push(request.url());
  });
  await page.addInitScript(() => {
    const active = new Set();
    window.workerAudit = { created: 0, terminated: 0, get active() { return active.size; } };
    window.Worker = new Proxy(window.Worker, {
      construct(Target, args) {
        const worker = new Target(...args);
        active.add(worker);
        window.workerAudit.created++;
        const terminate = worker.terminate.bind(worker);
        worker.terminate = () => {
          if (active.delete(worker)) window.workerAudit.terminated++;
          terminate();
        };
        return worker;
      },
    });
  });

  const ready = async (format, count) => {
    await page.waitForFunction(({ format, count }) => {
      const app = window.vue2Fixture?.app;
      return app?.ready.length === count && app.ready[count - 1].format === format;
    }, { format, count });
  };
  const rendered = async (kind) => {
    await page.waitForFunction(kind => {
      const shadow = document.querySelector('#primary .office-viewer-surface')?.shadowRoot;
      const node = shadow?.querySelector({ word: '.page', excel: '.cell', ppt: '.slide', pdf: 'canvas' }[kind]);
      if (!node || node.getBoundingClientRect().width <= 0) return false;
      return kind === 'pdf' ? node.width > 300 : node.textContent.trim().length > 0;
    }, kind);
  };

  try {
    await page.goto(`${base}?assets=${assetMode}`);
    await page.waitForFunction(() => window.vue2Fixture?.app.$refs.preview?._viewer);
    assert.equal(await page.evaluate(() => window.vue2Fixture.version), '2.7.16');
    let count = 0;
    for (const [kind, format] of Object.entries({ word: 'docx', excel: 'xlsx', ppt: 'pptx', pdf: 'pdf' })) {
      await page.locator(`[data-sample="${kind}"]`).click();
      await ready(format, ++count);
      await rendered(kind);
      const result = await page.evaluate(() => window.vue2Fixture.app.ready.at(-1));
      assert.ok(Object.keys(result.stats).length > 0);
      suite.results.push({ test: `render-${kind}`, result });
      if (mode === 'production' && assetMode === 'bundled') {
        await page.screenshot({ path: resolve(output, `${kind}.png`) });
      }
    }

    // Real File input takes the same prop/watcher path as the sample buttons.
    await page.locator('#file').setInputFiles(resolve(consumer, 'public/samples/styles-ppt.pptx'));
    await ready('pptx', ++count);
    await rendered('ppt');
    suite.results.push({ test: 'file-input' });
    const width = await page.locator('#primary .office-viewer-surface').evaluate(surface => surface.shadowRoot.querySelector('.slide').getBoundingClientRect().width);
    await page.locator('#zoom').click();
    await page.waitForFunction(width => {
      const node = document.querySelector('#primary .office-viewer-surface').shadowRoot.querySelector('.slide');
      return Math.abs(node.getBoundingClientRect().width / width - 1.5) < 0.03;
    }, width);
    await page.locator('#fit').click();
    assert.equal(await page.evaluate(() => window.vue2Fixture.app.$refs.preview._viewer.getZoom().mode), 'fit-width');
    await page.locator('#visibility').click();
    await page.locator('#primary').waitFor({ state: 'hidden' });
    await page.locator('#visibility').click();
    await rendered('ppt');
    suite.results.push({ test: 'zoom-and-hidden-container' });

    await page.evaluate(async () => {
      const app = window.vue2Fixture.app;
      const [word, ppt] = await Promise.all([app.sample('word'), app.sample('ppt')]);
      let release;
      class SlowBlob extends Blob {
        arrayBuffer() { return new Promise(resolve => { release = () => resolve(word.bytes); }); }
      }
      app.source = { bytes: new SlowBlob([word.bytes]), name: word.name };
      await app.$nextTick();
      if (!release) throw new Error('Pending Blob did not start loading');
      app.source = ppt;
      await app.$nextTick();
      release();
    });
    await ready('pptx', ++count);
    await rendered('ppt');
    suite.results.push({ test: 'prop-replacement-cancels-pending-file' });
    await page.locator('#clear').click();
    await page.waitForFunction(() => !document.querySelector('#primary .office-viewer-surface').shadowRoot.querySelector('.slide'));
    suite.results.push({ test: 'clear' });

    await page.locator('#file').setInputFiles({ name: 'unsupported.txt', mimeType: 'text/plain', buffer: Buffer.from('unsupported document') });
    await page.waitForFunction(() => window.vue2Fixture.app.errors.length === 1);
    assert.deepEqual(await page.evaluate(() => window.vue2Fixture.app.errors.map(error => error.code)), ['UNSUPPORTED_FORMAT']);
    await page.locator('[data-sample="pdf"]').click();
    await ready('pdf', ++count);
    await rendered('pdf');
    assert.equal(await page.evaluate(() => window.vue2Fixture.app.errors.length), 1, 'One error must not emit twice');
    suite.results.push({ test: 'error-deduplication-and-recovery' });

    await page.evaluate(async () => {
      const app = window.vue2Fixture.app;
      const word = await app.sample('word');
      class SlowBlob extends Blob {
        arrayBuffer() { return new Promise(resolve => { window.releaseVue2Read = () => resolve(word.bytes); }); }
      }
      window.destroyedVue2Viewer = app.$refs.preview._viewer;
      app.source = { bytes: new SlowBlob([word.bytes]), name: word.name };
      await app.$nextTick();
      if (!window.releaseVue2Read) throw new Error('Pending Blob did not start before unmount');
    });
    await page.locator('#mount').click();
    await page.waitForFunction(() => window.workerAudit.active === 0 && !document.querySelector('#primary .office-preview'));
    const code = await page.evaluate(async () => {
      window.releaseVue2Read();
      const app = window.vue2Fixture.app;
      const source = await app.sample('ppt');
      const code = await window.destroyedVue2Viewer.open(source).then(() => 'BAD', error => error.code);
      app.source = source;
      await app.$nextTick();
      return code;
    });
    assert.equal(code, 'DESTROYED');
    await page.locator('#mount').click();
    await ready('pptx', ++count);
    await rendered('ppt');
    suite.results.push({ test: 'beforeDestroy-during-load-and-remount', code });

    await page.locator('#second').click();
    await page.waitForFunction(() => window.vue2Fixture.app.secondResult?.format === 'pdf'
      && document.querySelector('#secondary .office-viewer-surface')?.shadowRoot.querySelector('canvas')?.width > 300);
    await page.locator('#mount').click();
    await page.waitForFunction(() => window.workerAudit.active === 1);
    assert.ok(await page.locator('#secondary .office-viewer-surface').evaluate(surface => surface.shadowRoot.querySelector('canvas').width > 300));
    await page.locator('#second').click();
    await page.waitForFunction(() => window.workerAudit.active === 0);
    const audit = await page.evaluate(() => ({ ...window.workerAudit }));
    assert.equal(audit.created, audit.terminated);
    suite.results.push({ test: 'independent-instances-and-worker-cleanup', ...audit });
    assert.deepEqual(await page.evaluate(() => window.vue2Fixture.app.errors.map(error => error.code)), ['UNSUPPORTED_FORMAT']);
    assert.deepEqual(suite.errors, [], 'Unhandled browser or console errors');
    assert.deepEqual(suite.failures, [], 'Failed resource requests');
    assert.deepEqual(suite.httpErrors, [], 'HTTP resource errors');
    assert.ok(suite.assetRequests.some(url => url.includes('.wasm')), 'Missing real WASM requests');
    if (assetMode === 'static') {
      assert.ok(suite.assetRequests.some(url => url.includes('/vue2/office-assets/pdf/pdfium/pdfium.std.wasm')));
    }
    suite.passed = true;
    console.log(`Passed Vue 2 ${mode}/${assetMode}: ${suite.results.length} checks`);
  } catch (error) {
    suite.failure = error.message;
    throw error;
  } finally {
    await page.close();
  }
}

try {
  const dev = await vite.createServer({ root: consumer, server: { host: '127.0.0.1', port: 0 } });
  try {
    await dev.listen();
    const base = `http://127.0.0.1:${dev.httpServer.address().port}/vue2/`;
    for (const assets of ['bundled', 'static']) await verify(base, 'development', assets);
  } finally { await dev.close(); }
  const production = await vite.preview({ root: consumer, preview: { host: '127.0.0.1', port: 0 } });
  try {
    const base = `http://127.0.0.1:${production.httpServer.address().port}/vue2/`;
    for (const assets of ['bundled', 'static']) await verify(base, 'production', assets);
  } finally { await new Promise(resolve => production.httpServer.close(resolve)); }
} finally {
  await writeFile(resolve(output, 'results.json'), JSON.stringify(report, null, 2));
  await writeFile(resolve(root, 'output/vue2-consumer.json'), JSON.stringify({ consumer }, null, 2));
  await browser.close();
}
