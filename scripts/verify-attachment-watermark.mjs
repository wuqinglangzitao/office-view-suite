import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const { consumer: packed } = JSON.parse(await readFile(resolve(root, 'output/packed-consumer.json')));
const consumer = await realpath(await mkdtemp(resolve(tmpdir(), 'office-attachment-watermark-')));
const names = ['viewer-core', 'viewer-word', 'viewer-excel', 'viewer-ppt', 'viewer-pdf', 'viewer'];
const dependencies = Object.fromEntries(await Promise.all(names.map(async name => {
  const { version } = JSON.parse(await readFile(resolve(root, 'packages', name, 'package.json')));
  return [`@office-view-suite/${name}`, `file:./tarballs/office-view-suite-${name}-${version}.tgz`];
})));
await cp(resolve(packed, 'tarballs'), resolve(consumer, 'tarballs'), { recursive: true });
await cp(resolve(packed, 'public/samples'), resolve(consumer, 'public/samples'), { recursive: true });
await cp(resolve(root, 'docs/examples/attachment-preview'), resolve(consumer, 'src/attachment-preview'), { recursive: true });
await writeFile(resolve(consumer, 'package.json'), JSON.stringify({
  private: true, type: 'module',
  dependencies: { ...dependencies, vue: '2.7.16', 'element-ui': '2.15.3' },
  devDependencies: { vite: '6.4.4', '@vitejs/plugin-vue2': '2.3.4' },
  pnpm: { overrides: dependencies },
}, null, 2));
await writeFile(resolve(consumer, 'index.html'), '<html><head><link rel="icon" href="data:,"></head><body><div id="app"></div><script type="module" src="/src/main.js"></script></body></html>');
await writeFile(resolve(consumer, 'vite.config.js'), `import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue2';
export default defineConfig({ plugins: [vue()], optimizeDeps: { exclude: ${JSON.stringify(names.map(name => `@office-view-suite/${name}`))} }, build: { target: 'es2022' } });`);
await writeFile(resolve(consumer, 'src/main.js'), `import Vue from 'vue';
import ElementUI from 'element-ui';
import 'element-ui/lib/theme-chalk/index.css';
import { openAttachmentPreview, closeAttachmentPreview } from './attachment-preview/service.js';
Vue.use(ElementUI);
const owner = new Vue({ render: h => h('div') }).$mount('#app');
window.attachmentHarness = {
  owner, dialog: null, events: [], fetches: 0, watermarkDone: false,
  open(watermark) {
    const harness = this;
    this.dialog = openAttachmentPreview({ parent: owner, fileUrl: '/samples/styles-word.docx', fileName: 'styles-word.docx', watermark,
      async fetchFile({ fileUrl, signal }) {
        harness.fetches++;
        if (harness.delayFetch) await new Promise((resolve, reject) => {
          harness.releaseFetch = resolve;
          signal.addEventListener('abort', () => reject(new DOMException('cancelled', 'AbortError')), { once: true });
        });
        const response = await fetch(fileUrl, { signal });
        return response.blob();
      },
    });
    for (const name of ['ready', 'error', 'watermark-error']) this.dialog.$on(name, value => this.events.push({ name, code: value.code, format: value.format }));
    return this.dialog;
  },
  close: closeAttachmentPreview,
};`);
execFileSync('pnpm', ['install', '--prefer-offline', '--ignore-scripts'], { cwd: consumer, stdio: 'inherit' });
execFileSync('pnpm', ['exec', 'vite', 'build'], { cwd: consumer, stdio: 'inherit' });
const vite = await import(pathToFileURL(resolve(consumer, 'node_modules/vite/dist/node/index.js')));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const output = resolve(root, 'output/playwright/attachment-watermark');
await mkdir(output, { recursive: true });
const report = { consumer, suites: [] };

async function verify(base, mode) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const suite = { mode, checks: [], errors: [], passed: false };
  report.suites.push(suite);
  page.on('pageerror', error => suite.errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') suite.errors.push(message.text()); });
  await page.addInitScript(() => {
    const active = new Set();
    window.workerAudit = { created: 0, get active() { return active.size; } };
    window.Worker = new Proxy(window.Worker, {
      construct(Target, args) {
        const worker = new Target(...args);
        active.add(worker);
        window.workerAudit.created++;
        const terminate = worker.terminate.bind(worker);
        worker.terminate = () => { active.delete(worker); terminate(); };
        return worker;
      },
    });
  });
  try {
    await page.goto(base);
    await page.waitForFunction(() => window.attachmentHarness);
    await page.evaluate(() => {
      const harness = window.attachmentHarness;
      harness.delayFetch = true;
      const dialog = harness.open({ content: '初始用户' });
      harness.pendingInitial = dialog.setWatermark({ content: '懒挂载新用户' }).then(() => { harness.watermarkDone = true; });
    });
    await page.waitForFunction(() => window.attachmentHarness.watermarkDone && window.attachmentHarness.releaseFetch);
    assert.equal(await page.evaluate(() => window.attachmentHarness.events.filter(event => event.name === 'ready').length), 0);
    assert.equal(await page.evaluate(() => window.attachmentHarness.dialog.getWatermark().content[0]), '懒挂载新用户');
    await page.evaluate(() => window.attachmentHarness.releaseFetch());
    await page.waitForFunction(() => window.attachmentHarness.events.some(event => event.name === 'ready'));
    suite.checks.push('lazy-setter-commits-before-download');

    const before = await page.evaluate(() => {
      const harness = window.attachmentHarness;
      const child = harness.dialog.$refs.preview;
      child.setZoom(1.5);
      child.$refs.officeHost.scrollTop = 90;
      return { workers: window.workerAudit.created, fetches: harness.fetches, zoom: child.zoomScale, scroll: child.$refs.officeHost.scrollTop };
    });
    await page.evaluate(async () => { await window.attachmentHarness.dialog.setWatermark({ content: '运行更新用户' }); });
    assert.deepEqual(await page.evaluate(() => {
      const harness = window.attachmentHarness;
      const child = harness.dialog.$refs.preview;
      return { workers: window.workerAudit.created, fetches: harness.fetches, zoom: child.zoomScale, scroll: child.$refs.officeHost.scrollTop };
    }), before);
    suite.checks.push('runtime-setter-preserves-download-worker-zoom-scroll');

    assert.equal(await page.evaluate(async () => {
      const harness = window.attachmentHarness;
      return harness.dialog.setWatermark({ content: '' }).then(() => 'BAD', error => error.code);
    }), 'INVALID_WATERMARK');
    assert.equal(await page.evaluate(() => window.attachmentHarness.dialog.getWatermark().content[0]), '运行更新用户');
    assert.equal(await page.evaluate(() => window.attachmentHarness.dialog.desiredWatermark.content[0]), '运行更新用户');
    assert.equal(await page.evaluate(() => window.attachmentHarness.dialog.$refs.preview.canZoom), true);
    assert.equal(await page.evaluate(() => window.attachmentHarness.events.filter(event => event.name === 'error').length), 0);
    suite.checks.push('failed-update-rolls-back-with-independent-error');
    await page.screenshot({ path: resolve(output, `${mode}.png`) });

    await page.evaluate(() => {
      const harness = window.attachmentHarness;
      window.originalAttachmentDecode = HTMLImageElement.prototype.decode;
      HTMLImageElement.prototype.decode = function () {
        return window.originalAttachmentDecode.call(this).then(() => new Promise(resolve => { window.releaseAttachmentDecode = resolve; }));
      };
      harness.pendingClose = harness.dialog.setWatermark({ content: '等待关闭的用户' }).then(() => 'BAD', error => error.code);
    });
    await page.waitForFunction(() => window.releaseAttachmentDecode);
    await page.evaluate(() => { window.attachmentHarness.owner.$destroy(); });
    assert.equal(await page.evaluate(() => window.attachmentHarness.pendingClose), 'DESTROYED');
    await page.waitForFunction(() => window.workerAudit.active === 0 && !document.querySelector('.office-viewer-host'));
    await page.evaluate(() => {
      HTMLImageElement.prototype.decode = window.originalAttachmentDecode;
      window.releaseAttachmentDecode();
    });
    suite.checks.push('owner-destroy-cancels-pending-setter-and-workers');
    assert.deepEqual(suite.errors, []);
    suite.passed = true;
    console.log(`Passed attachment watermark ${mode}: ${suite.checks.length} checks`);
  } catch (error) {
    suite.failure = error.message;
    throw error;
  } finally { await page.close(); }
}

try {
  const dev = await vite.createServer({ root: consumer, server: { host: '127.0.0.1', port: 0 } });
  try {
    await dev.listen();
    await verify(`http://127.0.0.1:${dev.httpServer.address().port}/`, 'development');
  } finally { await dev.close(); }
  const production = await vite.preview({ root: consumer, preview: { host: '127.0.0.1', port: 0 } });
  try { await verify(`http://127.0.0.1:${production.httpServer.address().port}/`, 'production'); }
  finally { await new Promise(resolve => production.httpServer.close(resolve)); }
} finally {
  await writeFile(resolve(output, 'results.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
