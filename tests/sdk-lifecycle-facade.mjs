import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import test from 'node:test';
import { formatBytes } from './sdk-format-fixtures.mjs';

const turn = () => new Promise(resolve => setImmediate(resolve));
const rejectedWith = code => error => error.code === code;
const input = format => ({ format, bytes: formatBytes(format) });
let releaseExcelImport;
globalThis.__sdkMockImports = { excel: new Promise(resolve => { releaseExcelImport = resolve; }) };
registerHooks({
  resolve(specifier, context, nextResolve) {
    const kind = /^@office-view-suite\/viewer-(word|excel|ppt|pdf)$/.exec(specifier)?.[1];
    if (!kind) return nextResolve(specifier, context);
    const name = kind[0].toUpperCase() + kind.slice(1);
    const code = `await globalThis.__sdkMockImports?.[${JSON.stringify(kind)}]; export function create${name}Adapter(options) { return globalThis.__sdkAdapterFactory(${JSON.stringify(kind)}, options); }`;
    return { url: `data:text/javascript,${encodeURIComponent(code)}`, shortCircuit: true };
  },
});
const { createOfficeViewer } = await import('../packages/viewer/dist/index.js');

class Element {
  children = [];
  style = { setProperty(name, value) { this[name] = value; }, removeProperty(name) { delete this[name]; } };
  scrollWidth = 640;
  get clientWidth() { return this.parent?.clientWidth ?? this.width; }
  width = 640;
  height = 0;
  get clientHeight() { return this.parent?.clientHeight ?? this.height; }
  set clientHeight(value) { this.height = value; }
  ownerDocument = { createElement: () => new Element() };
  appendChild(child) { this.children.push(child); child.parent = this; return child; }
  replaceChildren(...children) { this.children = []; for (const child of children) this.appendChild(child); }
  attachShadow() { this.shadowRoot = { children: [], get childNodes() { return this.children; }, querySelectorAll() { return []; }, replaceChildren(...children) { this.children = children; } }; return this.shadowRoot; }
  getBoundingClientRect() { return { width: this.width, height: this.clientHeight }; }
  remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
}

const observers = [];
globalThis.ResizeObserver = class {
  constructor(callback) { this.callback = callback; observers.push(this); }
  observe(target) { this.target = target; }
  disconnect() { this.disconnected = true; }
  notify(width, height = 300) { this.callback([{ target: this.target, contentRect: { width, height } }]); }
};
const frames = new Map();
let nextFrame = 0;
globalThis.requestAnimationFrame = callback => { frames.set(++nextFrame, callback); return nextFrame; };
globalThis.cancelAnimationFrame = id => frames.delete(id);
function flushFrames() { for (const [id, callback] of frames) { frames.delete(id); callback(); } }

function runtime({ open, resize = true, contentWidth, contentZoom = false } = {}) {
  const calls = { created: 0, destroyed: 0, rendered: 0, disposed: 0, resized: 0, contentWidth, options: [], kinds: [] };
  globalThis.__sdkAdapterFactory = (kind, options) => {
    calls.created++;
    calls.kinds.push(kind);
    calls.options.push(options);
    return {
      open: open ?? (async request => ({ format: request.format, document: { label: 'document' }, stats: { sheets: 2 }, durationMs: 1, warnings: [{ code: 'FONT', message: 'fallback' }] })),
      render(host, document, onError, onContentChange) {
        calls.rendered++;
        calls.document = document;
        calls.host = host;
        calls.onError = onError;
        calls.onContentChange = onContentChange;
        host.shadowRoot.replaceChildren(document);
        return {
          stats: { pages: 3 },
          dispose() { calls.disposed++; host.shadowRoot.replaceChildren(); },
          ...(resize ? { resize() { calls.resized++; } } : {}),
          ...(contentWidth ? { getContentWidth: () => calls.contentWidth } : {}),
          ...(contentZoom ? { setZoom(scale) { calls.contentScale = scale; } } : {}),
        };
      },
      destroy() { calls.destroyed++; },
    };
  };
  return calls;
}

test('facade cancellation settles during lazy adapter loading and creates no adapter after destroy', async () => {
  const calls = runtime();
  const viewer = createOfficeViewer(new Element());
  const opening = viewer.open(input('xlsx'));
  const rejected = assert.rejects(opening, rejectedWith('DESTROYED'));
  await turn();
  viewer.destroy();
  await rejected;
  releaseExcelImport();
  await turn();
  assert.equal(calls.created, 0);
  await assert.rejects(viewer.open(input('xlsx')), rejectedWith('DESTROYED'));
});

test('facade owns only its child and listener exceptions cannot break open or cleanup', async () => {
  const calls = runtime();
  const container = new Element();
  const sibling = container.appendChild(new Element());
  const originalShadow = container.attachShadow();
  originalShadow.replaceChildren('host content');
  container.style.color = 'red';
  const options = { assets: { baseUrl: '/static/office/' } };
  const viewer = createOfficeViewer(container, options);
  const events = [];
  viewer.on(() => { throw new Error('consumer bug'); });
  viewer.on(event => events.push(event));
  const result = await viewer.open(input('docx'));
  assert.equal(calls.options[0], options);
  assert.equal(result.pageCount, 3);
  assert.equal(result.itemCount, 2);
  assert.deepEqual(result.warnings, [{ code: 'FONT', message: 'fallback' }]);
  assert.ok(events.some(event => event.type === 'warning' && event.code === 'FONT'));
  assert.equal(container.children.length, 2);
  assert.deepEqual(originalShadow.children, ['host content']);
  viewer.clear();
  assert.equal(calls.disposed, 1);
  viewer.destroy();
  await turn();
  assert.deepEqual(container.children, [sibling]);
  assert.equal(container.style.color, 'red');
  assert.equal(calls.destroyed, 1);
  assert.equal(observers.at(-1).disconnected, true);
});

test('width changes coalesce resize without rerendering on height changes', async () => {
  const calls = runtime();
  const container = new Element();
  const viewer = createOfficeViewer(container);
  await viewer.open(input('docx'));
  const observer = observers.at(-1);
  observer.notify(640, 300);
  observer.notify(640, 1200);
  flushFrames();
  assert.equal(calls.resized, 0);
  assert.equal(calls.rendered, 1);
  container.width = 700;
  observer.notify(680);
  observer.notify(700);
  flushFrames();
  assert.equal(calls.resized, 1);
  assert.equal(calls.rendered, 1);
  viewer.resize();
  assert.equal(calls.resized, 2);
  container.width = 800;
  observer.notify(800);
  viewer.destroy();
  flushFrames();
  assert.equal(calls.resized, 2);
});

test('fallback resize rerenders only on width change, and each instance owns its adapter', async () => {
  const calls = runtime({ resize: false });
  const firstContainer = new Element();
  const first = createOfficeViewer(firstContainer);
  const second = createOfficeViewer(new Element());
  await Promise.all([first.open(input('docx')), second.open(input('docx'))]);
  assert.equal(calls.created, 2);
  const observer = observers.at(-2);
  firstContainer.width = 720;
  observer.notify(720);
  flushFrames();
  assert.equal(calls.rendered, 3);
  assert.equal(calls.disposed, 1);
  first.destroy();
  await turn();
  assert.equal(calls.destroyed, 1);
  await second.open(input('docm'));
  assert.equal(calls.created, 2, 'docx and docm use one Word adapter per viewer');
  second.destroy();
});

test('pre-aborted and delayed Blob requests do not initialize adapters', async () => {
  const calls = runtime();
  const viewer = createOfficeViewer(new Element());
  await assert.rejects(viewer.open({ ...input('docx'), signal: AbortSignal.abort() }), rejectedWith('ABORTED'));
  let finishRead;
  class SlowBlob extends Blob { arrayBuffer() { return new Promise(resolve => { finishRead = resolve; }); } }
  const opening = viewer.open({ bytes: new SlowBlob(['file']), format: 'docx' });
  const rejected = assert.rejects(opening, rejectedWith('ABORTED'));
  viewer.clear();
  await rejected;
  finishRead(new Uint8Array([1, 2, 3]).buffer);
  await turn();
  assert.equal(calls.created, 0);
  viewer.destroy();
});

test('late parsed documents cannot overwrite replacements or cleared content', async () => {
  const pending = [];
  const calls = runtime({ open: request => new Promise(resolve => pending.push(() => resolve({ format: request.format, document: { label: request.name }, stats: {}, warnings: [], durationMs: 1 }))) });
  const viewer = createOfficeViewer(new Element());
  const first = viewer.open({ ...input('docx'), name: 'old.docx' });
  const rejected = assert.rejects(first, rejectedWith('ABORTED'));
  await turn();
  const second = viewer.open({ ...input('docx'), name: 'new.docx' });
  await rejected;
  await turn();
  pending[1]();
  await second;
  pending[0]();
  await turn();
  assert.equal(calls.rendered, 1);
  assert.equal(calls.document.label, 'new.docx');
  const lateError = calls.onError;
  const events = [];
  viewer.on(event => events.push(event));
  viewer.clear();
  lateError(new Error('old image failed'));
  assert.equal(events.filter(event => event.type === 'error').length, 0);
  viewer.destroy();
});

test('failed adapter creation is not cached and render errors use RENDER_FAILED', async () => {
  const calls = runtime();
  const factory = globalThis.__sdkAdapterFactory;
  let attempts = 0;
  globalThis.__sdkAdapterFactory = (...args) => { if (++attempts === 1) throw new Error('asset setup'); return factory(...args); };
  const viewer = createOfficeViewer(new Element());
  await assert.rejects(viewer.open(input('docx')), rejectedWith('ASSET_LOAD_FAILED'));
  await viewer.open(input('docx'));
  assert.equal(calls.rendered, 1);
  viewer.destroy();
  globalThis.__sdkAdapterFactory = (...args) => ({ ...factory(...args), render() { throw new Error('layout failed'); } });
  const broken = createOfficeViewer(new Element());
  await assert.rejects(broken.open(input('docx')), rejectedWith('RENDER_FAILED'));
  broken.destroy();
});

test('loading listeners can clear, destroy, or replace the request without stale work', async () => {
  for (const action of ['clear', 'destroy', 'replace']) {
    const calls = runtime();
    const viewer = createOfficeViewer(new Element());
    let replacement;
    let acted = false;
    viewer.on(event => {
      if (event.type !== 'statechange' || event.state !== 'loading' || acted) return;
      acted = true;
      if (action === 'clear') viewer.clear();
      if (action === 'destroy') viewer.destroy();
      if (action === 'replace') replacement = viewer.open(input('docm'));
    });
    await assert.rejects(viewer.open(input('docx')), rejectedWith(action === 'destroy' ? 'DESTROYED' : 'ABORTED'));
    if (replacement) await replacement;
    assert.equal(calls.rendered, action === 'replace' ? 1 : 0);
    viewer.destroy();
  }
});

test('a host shown again at the same width resumes incremental rendering', async () => {
  const calls = runtime();
  const host = new Element();
  const viewer = createOfficeViewer(host);
  await viewer.open(input('docx'));
  const observer = observers.at(-1);
  host.width = 0;
  observer.notify(0);
  flushFrames();
  host.width = 640;
  observer.notify(640);
  flushFrames();
  assert.equal(calls.resized, 1);
  viewer.destroy();
});

test('content routing uses all four adapter factories and rejects conflicts before importing', async () => {
  const calls = runtime();
  const viewer = createOfficeViewer(new Element());
  await assert.rejects(viewer.open({ bytes: formatBytes('xlsx'), format: 'pdf' }), rejectedWith('INVALID_SOURCE'));
  assert.equal(calls.created, 0);
  for (const [format, kind] of [['docx', 'word'], ['xlsx', 'excel'], ['pptx', 'ppt'], ['pdf', 'pdf']]) {
    const result = await viewer.open({ bytes: formatBytes(format), name: 'wrong.doc' });
    assert.equal(result.format, format);
    assert.equal(calls.kinds.at(-1), kind);
  }
  assert.equal(calls.created, 4);
  viewer.destroy();
});

test('zoom preserves rendered content and adapts its viewport without modifying the caller', async () => {
  const calls = runtime({ contentWidth: 1000 });
  const container = new Element();
  container.style.color = 'red';
  const viewer = createOfficeViewer(container);
  assert.deepEqual(viewer.getCapabilities(), { zoom: true, fitWidth: true });
  assert.deepEqual(viewer.getZoom(), { mode: 'fixed', scale: 1 });
  viewer.setZoom(2);
  await viewer.open(input('xlsx'));
  assert.equal(calls.host.shadowRoot.children[0].style.zoom, '2');
  assert.equal(calls.host.style.width, '100%');
  assert.equal(calls.host.shadowRoot.children[0].style.width, '1000px');
  assert.equal(calls.host.style['--office-viewer-viewport-width'], '320px');
  viewer.setZoom('fit-width');
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 0.64 });
  assert.equal(calls.host.style.width, '100%');
  assert.equal(calls.host.shadowRoot.children[0].style.width, '1000px');
  container.width = 800;
  observers.at(-1).notify(800);
  flushFrames();
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 0.8 });
  assert.equal(calls.rendered, 1);
  assert.equal(calls.disposed, 0);
  assert.equal(calls.resized, 3);
  for (const invalid of [0, -1, 4.01, NaN, Infinity, '50%', null]) {
    assert.throws(() => viewer.setZoom(invalid), rejectedWith('INVALID_SOURCE'));
  }
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 0.8 });
  viewer.clear();
  await viewer.open(input('docx'));
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 0.8 });
  assert.equal(container.style.zoom, undefined);
  assert.equal(container.style.color, 'red');
  viewer.destroy();
  assert.deepEqual(viewer.getCapabilities(), { zoom: false, fitWidth: false });
  assert.throws(() => viewer.setZoom(1), rejectedWith('DESTROYED'));
});

test('fixed zoom remains observable while the viewer is hidden', () => {
  runtime();
  const container = new Element();
  container.width = 0;
  const viewer = createOfficeViewer(container);
  viewer.setZoom(1.5);
  assert.deepEqual(viewer.getZoom(), { mode: 'fixed', scale: 1.5 });
  viewer.destroy();
});

test('Excel measures available height on open without requiring a zoom call', async () => {
  const calls = runtime({ contentZoom: true });
  const container = new Element();
  container.clientHeight = 700;
  const viewer = createOfficeViewer(container);
  await viewer.open(input('xlsx'));
  assert.equal(calls.host.style['--office-viewer-viewport-height'], '700px');
  assert.equal(calls.resized, 1);
  assert.equal(container.children.length, 1, 'height probes never remain in caller DOM');
  container.clientHeight = 0;
  viewer.resize();
  assert.equal(calls.host.style['--office-viewer-viewport-height'], undefined);
  assert.equal(container.children.length, 1);
  viewer.destroy();
});

test('content zoom keeps worksheet chrome at container size and follows height-only resizing', async () => {
  const calls = runtime({ contentWidth: 2000, contentZoom: true });
  const container = new Element();
  container.clientHeight = 507;
  const viewer = createOfficeViewer(container);
  await viewer.open(input('xlsx'));
  viewer.setZoom(0.5);
  assert.equal(calls.contentScale, 0.5);
  assert.equal(calls.host.shadowRoot.children[0].style.zoom, '1');
  assert.equal(calls.host.style.width, '100%');
  assert.equal(calls.host.shadowRoot.children[0].style.width, '100%');
  assert.equal(calls.host.style['--office-viewer-viewport-height'], '507px');
  container.clientHeight = 350;
  observers.at(-1).notify(640, 350);
  flushFrames();
  assert.equal(calls.host.style['--office-viewer-viewport-height'], '350px');
  viewer.setZoom('fit-width');
  assert.equal(calls.contentScale, 0.32);
  assert.equal(calls.host.style.width, '100%');
  viewer.setZoom(2);
  assert.equal(calls.contentScale, 2);
  assert.equal(calls.host.shadowRoot.children[0].style.zoom, '1');
  assert.equal(calls.rendered, 1);
  viewer.destroy();
});

test('paged zoom keeps the surface at container width while preserving page layout and scale', async () => {
  const calls = runtime();
  const container = new Element();
  const viewer = createOfficeViewer(container);
  await viewer.open(input('docx'));
  calls.host.shadowRoot.querySelectorAll = () => [{
    getBoundingClientRect: () => ({ width: 400 * Number(calls.host.shadowRoot.children[0].style.zoom || 1) }),
  }];
  viewer.setZoom(0.5);
  assert.equal(calls.host.style.width, '100%');
  assert.equal(calls.host.shadowRoot.children[0].style.width, '1280px');
  viewer.setZoom(2);
  assert.equal(calls.host.style.width, '100%');
  assert.equal(calls.host.shadowRoot.children[0].style.width, '400px');
  viewer.setZoom('fit-width');
  assert.equal(viewer.getZoom().scale, 1.6);
  assert.equal(calls.rendered, 1);
  viewer.destroy();
});

test('fit width follows active worksheet changes and ignores stale render notifications', async () => {
  const calls = runtime({ contentWidth: 1000 });
  const viewer = createOfficeViewer(new Element());
  await viewer.open(input('xlsx'));
  viewer.setZoom('fit-width');
  calls.contentWidth = 2000;
  calls.onContentChange();
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 0.32 });
  assert.equal(calls.rendered, 1);
  const staleChange = calls.onContentChange;
  await viewer.open(input('docx'));
  const resizeCount = calls.resized;
  staleChange();
  assert.equal(calls.resized, resizeCount);
  viewer.destroy();
});

test('page fit uses document geometry even when the host is wider than the page', async () => {
  const calls = runtime();
  const container = new Element();
  const viewer = createOfficeViewer(container);
  await viewer.open(input('docx'));
  calls.host.shadowRoot.querySelectorAll = () => [{
    getBoundingClientRect: () => ({ width: 400 * Number(calls.host.shadowRoot.children[0].style.zoom || 1) }),
  }];
  viewer.setZoom('fit-width');
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 1.6 });
  container.width = 800;
  viewer.resize();
  assert.deepEqual(viewer.getZoom(), { mode: 'fit-width', scale: 2 });
  assert.equal(calls.rendered, 1);
  viewer.destroy();
});
