import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import test from 'node:test';
import vm from 'node:vm';
import { formatBytes } from './sdk-format-fixtures.mjs';

const require = createRequire(new URL('../packages/viewer-pdf/package.json', import.meta.url));
const ts = require('typescript');
const compile = source => ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const asModule = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const coreUrl = asModule(compile(await readFile(new URL('../packages/viewer-core/src/index.ts', import.meta.url), 'utf8')));
const { prepareOfficeRequest } = await import(coreUrl);
const pdfUrl = new URL('../packages/viewer-pdf/src/index.ts', import.meta.url);
const { createPdfAdapter } = await import(asModule(compile(await readFile(pdfUrl, 'utf8'))
  .replaceAll("'@office-view-suite/viewer-core'", JSON.stringify(coreUrl))
  .replaceAll('import.meta.url', JSON.stringify(pdfUrl.href))));

const tick = () => new Promise(resolve => setImmediate(resolve));
const bytes = () => new Uint8Array([37, 80, 68, 70, 45]);
class FakeWorker {
  messages = [];
  terminated = false;
  postMessage(message) { this.messages.push(message); }
  terminate() { this.terminated = true; }
  respond(message, extra) {
    this.onmessage?.({ data: { id: message.id, session: message.session, ...extra } });
  }
}
function setup(options = {}) {
  const workers = [];
  const calls = [];
  const adapter = createPdfAdapter({ ...options, workerFactory(url, workerOptions) {
    calls.push({ url, workerOptions });
    const worker = new FakeWorker();
    workers.push(worker);
    return worker;
  } });
  return { adapter, workers, calls };
}
async function openPdf(adapter, workers, pages = [{ wPt: 400, hPt: 600 }]) {
  const promise = adapter.open({ bytes: bytes(), format: 'pdf' });
  await tick();
  const worker = workers.at(-1);
  worker.respond(worker.messages.at(-1), { type: 'meta', pages });
  return promise;
}

test('PDF creates worker lazily, honors assets override and preserves caller bytes', async () => {
  const { adapter, workers, calls } = setup({ assets: { baseUrl: 'https://static.example/office', pdf: { workerUrl: 'https://app.example/custom-pdf.js' } } });
  assert.equal(workers.length, 0);
  const input = bytes();
  const opening = adapter.open({ bytes: input, format: 'pdf' });
  await tick();
  assert.equal(calls[0].url.href, 'https://app.example/custom-pdf.js');
  assert.deepEqual(calls[0].workerOptions, { type: 'classic' });
  const message = workers[0].messages[0];
  assert.equal(message.assets.pdfiumJsUrl, 'https://static.example/office/pdf/pdfium/pdfium.std.js');
  assert.notEqual(message.bytes, input.buffer);
  assert.equal(input.byteLength, 5);
  workers[0].respond(message, { type: 'meta', pages: [{ wPt: 400, hPt: 600 }] });
  const result = await opening;
  assert.equal(result.pageCount, 1);
  assert.equal(result.itemCount, 1);
  assert.ok(result.durationMs > 0);
  assert.deepEqual(result.warnings, []);
  adapter.destroy();
});

test('PDF honors worker URL override without custom workerFactory', async () => {
  const previous = globalThis.Worker;
  let url;
  let worker;
  globalThis.Worker = class extends FakeWorker { constructor(value) { super(); url = value; worker = this; } };
  try {
    const adapter = createPdfAdapter({ assets: { pdf: { workerUrl: 'https://app.example/override.js' } } });
    const opening = adapter.open({ bytes: bytes() });
    await tick();
    assert.equal(url.href, 'https://app.example/override.js');
    worker.respond(worker.messages[0], { type: 'meta', pages: [] });
    await opening;
    adapter.destroy();
  } finally { globalThis.Worker = previous; }
});

test('PDF transfers the facade-owned input without copying it again', async () => {
  const { adapter, workers } = setup();
  try {
    const original = bytes();
    const prepared = await prepareOfficeRequest({ bytes: original, format: 'pdf' });
    const opening = adapter.open(prepared);
    await tick();
    const message = workers[0].messages[0];
    assert.equal(message.bytes, prepared.bytes);
    assert.notEqual(message.bytes, original.buffer);
    structuredClone(message.bytes, { transfer: [message.bytes] });
    assert.equal(prepared.bytes.byteLength, 0);
    assert.equal(original.byteLength, 5);
    workers[0].respond(message, { type: 'meta', pages: [] });
    await opening;
  } finally { adapter.destroy(); }
});

test('PDF direct requests still apply the configured default format', async () => {
  const { adapter, workers } = setup({ format: 'docx' });
  try {
    await assert.rejects(adapter.open({ bytes: bytes() }), { code: 'UNSUPPORTED_FORMAT' });
    assert.equal(workers.length, 0);
    await openPdf(adapter, workers);
  } finally { adapter.destroy(); }
});

test('pre-abort and invalid input create no worker', async () => {
  const { adapter, workers } = setup();
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(adapter.open({ bytes: bytes(), signal: controller.signal }), { code: 'ABORTED' });
  await assert.rejects(adapter.open({ bytes: new ArrayBuffer(0) }), { code: 'INVALID_SOURCE' });
  await assert.rejects(adapter.open({ bytes: null }), { code: 'INVALID_SOURCE' });
  await assert.rejects(adapter.open({ bytes: Buffer.from('garbage'), format: 'pdf' }), { code: 'INVALID_SOURCE' });
  await assert.rejects(adapter.open({ bytes: formatBytes('xlsx'), name: 'wrong.pdf' }), { code: 'UNSUPPORTED_FORMAT' });
  await assert.rejects(adapter.open({ bytes: formatBytes('xlsx'), format: 'pdf' }), { code: 'INVALID_SOURCE' });
  assert.equal(workers.length, 0);
  adapter.destroy();
});

test('abort and destroy settle while Blob reading is pending', async () => {
  class SlowBlob extends Blob { arrayBuffer() { return new Promise(() => {}); } }
  for (const action of ['abort', 'destroy']) {
    const { adapter, workers } = setup();
    const controller = new AbortController();
    const opening = adapter.open({ bytes: new SlowBlob(['pdf']), signal: controller.signal });
    const rejection = assert.rejects(opening, { code: action === 'abort' ? 'ABORTED' : 'DESTROYED' });
    if (action === 'abort') controller.abort(); else adapter.destroy();
    await rejection;
    assert.equal(workers.length, 0);
    adapter.destroy();
  }
});

test('replacement terminates busy worker and ignores obsolete replies', async () => {
  const { adapter, workers } = setup();
  const first = adapter.open({ bytes: bytes() });
  const rejected = assert.rejects(first, { code: 'ABORTED' });
  await tick();
  const staleReply = workers[0].onmessage;
  const staleError = workers[0].onerror;
  const second = adapter.open({ bytes: bytes() });
  await rejected;
  await tick();
  assert.equal(workers[0].terminated, true);
  staleReply({ data: { type: 'meta', ...workers[0].messages[0], pages: [] } });
  staleError({ message: 'obsolete worker error' });
  assert.equal(workers[1].terminated, false);
  workers[1].respond(workers[1].messages[0], { type: 'meta', pages: [] });
  await second;
  adapter.destroy();
  await assert.rejects(adapter.open({ bytes: bytes() }), { code: 'DESTROYED' });
});

test('worker load errors are classified and subsequent open creates a fresh worker', async () => {
  const { adapter, workers } = setup();
  const first = adapter.open({ bytes: bytes() });
  const rejected = assert.rejects(first, { code: 'ASSET_LOAD_FAILED' });
  await tick();
  workers[0].onerror({ message: '404' });
  await rejected;
  assert.equal(workers[0].terminated, true);
  await openPdf(adapter, workers);
  assert.equal(workers.length, 2);
  adapter.destroy();
});

class Element {
  children = [];
  style = {};
  dataset = {};
  clientWidth = 632;
  paints = 0;
  append(...elements) { for (const element of elements) { element.parent = this; this.children.push(element); } }
  appendChild(element) { this.append(element); }
  replaceChildren(...elements) { this.children = []; this.append(...elements); }
  setAttribute() {}
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this); }
  attachShadow() { return this.shadowRoot = new Element(); }
  getContext() { return { putImageData: () => this.paints++ }; }
}

function installDom() {
  const names = ['document', 'IntersectionObserver', 'requestAnimationFrame', 'cancelAnimationFrame', 'devicePixelRatio'];
  const saved = Object.fromEntries(names.map(name => [name, globalThis[name]]));
  const observers = [];
  globalThis.document = { baseURI: 'https://app.example/office/', createElement: () => new Element() };
  globalThis.devicePixelRatio = 2;
  globalThis.IntersectionObserver = class {
    elements = [];
    constructor(callback, options) { this.callback = callback; this.options = options; observers.push(this); }
    observe(element) { this.elements.push(element); }
    disconnect() { this.disconnected = true; }
    show() { this.callback(this.elements.map(target => ({ target, isIntersecting: true }))); }
    setVisible(indices) {
      this.callback(this.elements.map((target, index) => ({ target, isIntersecting: indices.includes(index) })));
    }
  };
  globalThis.requestAnimationFrame = callback => setTimeout(callback, 0);
  globalThis.cancelAnimationFrame = clearTimeout;
  return { observers, restore() { for (const name of names) globalThis[name] = saved[name]; } };
}

test('PDF canvas sizing and resize preserve elements; disposal ignores late paint', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers);
    const host = new Element();
    const handle = adapter.render(host, result.document);
    const pages = host.shadowRoot.children[1];
    const canvas = pages.children[0];
    assert.equal(canvas.style.width, '600px');
    assert.equal(canvas.style.height, '900px');
    dom.observers[0].show();
    const worker = workers[0];
    const initial = worker.messages.at(-1);
    assert.equal(initial.width, 1200);
    worker.respond(initial, { type: 'page', page: 0, image: { width: 1200, height: 1800 } });
    await tick();
    assert.equal(canvas.paints, 1);
    pages.clientWidth = 432;
    handle.resize();
    await new Promise(resolve => setTimeout(resolve, 5));
    assert.equal(pages.children[0], canvas);
    assert.equal(canvas.style.width, '400px');
    const resized = worker.messages.at(-1);
    assert.equal(resized.width, 800);
    handle.dispose();
    assert.equal(canvas.width, 0);
    assert.equal(canvas.height, 0);
    adapter.clear();
    assert.equal(worker.terminated, true);
    worker.respond(resized, { type: 'page', page: 0, image: { width: 800, height: 1200 } });
    await tick();
    assert.equal(canvas.paints, 1);
    assert.equal(dom.observers[0].disconnected, true);
    assert.equal(host.shadowRoot.children.length, 0);
  } finally { adapter.destroy(); dom.restore(); }
});

test('PDF keeps only visible page bitmaps while preserving offscreen layout and rerenders on return', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers, Array.from({ length: 12 }, () => ({ wPt: 400, hPt: 600 })));
    const host = new Element();
    const handle = adapter.render(host, result.document);
    const pages = host.shadowRoot.children[1];
    const observer = dom.observers[0];
    const worker = workers[0];
    assert.equal(observer.options.rootMargin, '300px');
    assert.ok(pages.children.every(canvas => canvas.width === 0 && canvas.height === 0));
    for (const page of [...pages.children.keys(), 0]) {
      observer.setVisible([page]);
      const request = worker.messages.at(-1);
      assert.equal(request.type, 'render');
      assert.equal(request.page, page);
      worker.respond(request, { type: 'page', page, image: { width: 1200, height: 1800 } });
      await tick();
      assert.equal(pages.children.reduce((pixels, canvas) => pixels + canvas.width * canvas.height, 0), 1200 * 1800);
      for (const canvas of pages.children) {
        assert.equal(canvas.style.width, '600px');
        assert.equal(canvas.style.height, '900px');
      }
    }
    assert.equal(pages.children[0].paints, 2);
    handle.dispose();
    assert.ok(pages.children.every(canvas => canvas.width === 0 && canvas.height === 0));
  } finally { adapter.destroy(); dom.restore(); }
});

test('PDF discards a page response after it leaves the preload region', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers);
    const host = new Element();
    adapter.render(host, result.document);
    const canvas = host.shadowRoot.children[1].children[0];
    const observer = dom.observers[0];
    const worker = workers[0];
    observer.setVisible([0]);
    const stale = worker.messages.at(-1);
    observer.setVisible([]);
    worker.respond(stale, { type: 'page', page: 0, image: { width: 1200, height: 1800 } });
    await tick();
    assert.equal(canvas.paints, 0);
    assert.equal(canvas.width, 0);
    assert.equal(canvas.height, 0);
    observer.setVisible([0]);
    const current = worker.messages.at(-1);
    assert.notEqual(current.id, stale.id);
    worker.respond(current, { type: 'page', page: 0, image: { width: 1200, height: 1800 } });
    await tick();
    assert.equal(canvas.paints, 1);
    assert.equal(canvas.width, 1200);
  } finally { adapter.destroy(); dom.restore(); }
});

test('PDF reentry during an in-flight render uses the latest layout without duplicate requests', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers);
    const host = new Element();
    const handle = adapter.render(host, result.document);
    const pages = host.shadowRoot.children[1];
    const canvas = pages.children[0];
    const observer = dom.observers[0];
    const worker = workers[0];
    observer.setVisible([0]);
    const stale = worker.messages.at(-1);
    observer.setVisible([]);
    pages.clientWidth = 432;
    handle.resize();
    await new Promise(resolve => setTimeout(resolve, 5));
    observer.setVisible([0]);
    assert.equal(worker.messages.filter(message => message.type === 'render').length, 1);
    worker.respond(stale, { type: 'page', page: 0, image: { width: 1200, height: 1800 } });
    await tick();
    assert.equal(canvas.paints, 0);
    const current = worker.messages.at(-1);
    assert.equal(current.width, 800);
    worker.respond(current, { type: 'page', page: 0, image: { width: 800, height: 1200 } });
    await tick();
    assert.equal(canvas.paints, 1);
    assert.equal(canvas.width, 800);
    assert.equal(canvas.style.width, '400px');
  } finally { adapter.destroy(); dom.restore(); }
});

test('PDF still renders without IntersectionObserver and releases bitmaps on disposal', async () => {
  const dom = installDom();
  globalThis.IntersectionObserver = undefined;
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers, [{ wPt: 400, hPt: 600 }, { wPt: 600, hPt: 400 }]);
    const host = new Element();
    const handle = adapter.render(host, result.document);
    const canvases = host.shadowRoot.children[1].children;
    const worker = workers[0];
    const requests = worker.messages.filter(message => message.type === 'render');
    assert.equal(requests.length, 2);
    for (const request of requests) worker.respond(request, { type: 'page', page: request.page, image: { width: 1200, height: 1800 } });
    await tick();
    assert.ok(canvases.every(canvas => canvas.paints === 1 && canvas.width === 1200));
    handle.dispose();
    assert.ok(canvases.every(canvas => canvas.width === 0 && canvas.height === 0));
  } finally { adapter.destroy(); dom.restore(); }
});

test('PDF retries an evicted failed page on reentry without retrying continuously', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers);
    const errors = [];
    const host = new Element();
    adapter.render(host, result.document, error => errors.push(error));
    const canvas = host.shadowRoot.children[1].children[0];
    const observer = dom.observers[0];
    const worker = workers[0];
    observer.setVisible([0]);
    const failed = worker.messages.at(-1);
    worker.respond(failed, { type: 'error', code: 'RENDER_FAILED', message: 'bad page' });
    await tick();
    observer.setVisible([0]);
    assert.equal(worker.messages.at(-1), failed);
    assert.equal(errors.length, 1);
    observer.setVisible([]);
    observer.setVisible([0]);
    const retry = worker.messages.at(-1);
    assert.notEqual(retry.id, failed.id);
    worker.respond(retry, { type: 'page', page: 0, image: { width: 1200, height: 1800 } });
    await tick();
    assert.equal(canvas.paints, 1);
    assert.equal(canvas.dataset.error, undefined);
  } finally { adapter.destroy(); dom.restore(); }
});

test('clear retains idle initialized worker and abort terminates an in-flight open', async () => {
  const { adapter, workers } = setup();
  await openPdf(adapter, workers);
  adapter.clear();
  assert.equal(workers[0].terminated, false);
  await openPdf(adapter, workers);
  assert.equal(workers.length, 1);
  const controller = new AbortController();
  const opening = adapter.open({ bytes: bytes(), signal: controller.signal });
  const rejected = assert.rejects(opening, { code: 'ABORTED' });
  await tick();
  controller.abort();
  await rejected;
  assert.equal(workers[0].terminated, true);
  adapter.destroy();
});

test('PDF instances do not share document or worker state', async () => {
  const first = setup();
  const second = setup();
  await Promise.all([openPdf(first.adapter, first.workers), openPdf(second.adapter, second.workers)]);
  first.adapter.destroy();
  assert.equal(second.workers[0].terminated, false);
  await openPdf(second.adapter, second.workers);
  assert.equal(second.workers.length, 1);
  second.adapter.destroy();
});

test('background page errors are reported once and never become unhandled rejections', async () => {
  const dom = installDom();
  const { adapter, workers } = setup();
  try {
    const result = await openPdf(adapter, workers);
    const errors = [];
    adapter.render(new Element(), result.document, error => { errors.push(error); throw new Error('host callback'); });
    dom.observers[0].show();
    workers[0].respond(workers[0].messages.at(-1), { type: 'error', code: 'RENDER_FAILED', message: 'bad page' });
    await tick();
    assert.equal(errors.length, 1);
    assert.equal(errors[0].code, 'RENDER_FAILED');
  } finally { adapter.destroy(); dom.restore(); }
});

test('classic worker serializes initialization/open/render and rejects stale document sessions', async () => {
  const source = await readFile(new URL('../packages/viewer-pdf/src/worker/pdf-worker.js', import.meta.url), 'utf8');
  const replies = [];
  let currentDoc;
  const wasm_bindgen = async () => ({});
  wasm_bindgen.initialize_pdfium_render = () => true;
  wasm_bindgen.pageInfo = data => { currentDoc = data[0]; return [{ wPt: currentDoc, hPt: 20 }]; };
  wasm_bindgen.renderPage = () => ({ width: currentDoc, height: 20 });
  const sandbox = {
    URL, Uint8Array, Promise, JSON, Number, Error,
    self: { location: { href: 'https://app.example/sdk/worker.js' }, postMessage: message => replies.push(message) },
    importScripts() {}, fetch: async () => ({ ok: true, arrayBuffer: async () => new ArrayBuffer(1) }),
    PDFiumModule: async () => ({}), wasm_bindgen,
  };
  vm.runInNewContext(source, sandbox);
  const assets = { pdfiumJsUrl: './pdfium.js', glueJsUrl: './glue.js', pdfiumWasmUrl: './pdfium.wasm', glueWasmUrl: './glue.wasm' };
  const send = message => sandbox.self.onmessage({ data: { ...message, assets } });
  send({ type: 'open', id: 1, session: 1, bytes: new Uint8Array([10]).buffer });
  send({ type: 'render', id: 2, session: 1, page: 0, width: 100 });
  send({ type: 'open', id: 3, session: 2, bytes: new Uint8Array([30]).buffer });
  send({ type: 'render', id: 4, session: 1, page: 0, width: 100 });
  await tick();
  assert.deepEqual(replies.map(reply => reply.id), [1, 2, 3, 4]);
  assert.equal(replies[1].image.width, 10);
  assert.equal(replies[3].code, 'RENDER_FAILED');
});
