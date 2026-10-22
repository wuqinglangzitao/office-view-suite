import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from '../packages/viewer/node_modules/typescript/lib/typescript.js';
import { formatBytes } from './sdk-format-fixtures.mjs';

// Exercise the internal scheduler with controlled, genuinely pending decodes.
// Browser checks separately exercise Canvas, CSS parsing and published exports.
const source = await readFile(new URL('../packages/viewer/src/watermark.ts', import.meta.url), 'utf8');
const code = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
}).outputText.replace('@office-view-suite/viewer-core', new URL('../packages/viewer-core/dist/index.js', import.meta.url).href);
const moduleUrl = text => `data:text/javascript;base64,${Buffer.from(text).toString('base64')}`;
const controllerUrl = moduleUrl(code);
const { normalizeWatermark, validateWatermark, createWatermarkController } = await import(controllerUrl);
const controllerBridge = moduleUrl(`
  import { createWatermarkController as create, validateWatermark } from ${JSON.stringify(controllerUrl)};
  export { validateWatermark };
  export const createWatermarkController = (...args) => create(...args, globalThis.__watermarkFacadeRuntime);
`);
const layoutBridge = moduleUrl('export const createWatermarkLayout = () => ({ show() {}, hide() {}, request() {}, destroy() {} });');
let facadeCode = ts.transpileModule(await readFile(new URL('../packages/viewer/src/index.ts', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
}).outputText.replace('@office-view-suite/viewer-core', new URL('../packages/viewer-core/dist/index.js', import.meta.url).href)
  .replace('./watermark\'', `${controllerBridge}'`).replace('./watermark-layout\'', `${layoutBridge}'`);
for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
  const name = kind[0].toUpperCase() + kind.slice(1);
  facadeCode = facadeCode.replace(`@office-view-suite/viewer-${kind}`, moduleUrl(`export const create${name}Adapter = () => globalThis.__watermarkFacadeAdapter;`));
}
const { createOfficeViewer } = await import(moduleUrl(facadeCode));

const turn = () => new Promise(resolve => setImmediate(resolve));
const errorCode = code => error => error.code === code;
const options = content => ({ content });
const tile = content => ({ url: `data:image/png;${content}`, width: 180, height: 120 });
const codedError = code => Object.assign(new Error('simulated failure'), { code });

class AuditedTarget extends EventTarget {
  listeners = new Map();
  addEventListener(type, callback, ...rest) {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(callback);
    this.listeners.set(type, listeners);
    super.addEventListener(type, callback, ...rest);
  }
  removeEventListener(type, callback, ...rest) {
    this.listeners.get(type)?.delete(callback);
    super.removeEventListener(type, callback, ...rest);
  }
  get count() { return [...this.listeners.values()].reduce((sum, listeners) => sum + listeners.size, 0); }
}

function documentMock() {
  const win = new AuditedTarget();
  const fonts = new AuditedTarget();
  const media = [];
  win.devicePixelRatio = 1;
  win.matchMedia = () => { const target = new AuditedTarget(); media.push(target); return target; };
  const canvases = [];
  const doc = {
    defaultView: win,
    fonts,
    createElement(tag) {
      if (tag === 'span') return { style: { fontFamily: '' } };
      assert.equal(tag, 'canvas');
      const canvas = {
        width: 1, height: 1,
        getContext: () => ({ measureText: text => ({ width: [...text].length * doc.glyphWidth, actualBoundingBoxAscent: 12, actualBoundingBoxDescent: 3 }) }),
      };
      canvases.push(canvas);
      return canvas;
    },
    glyphWidth: 8,
  };
  return { doc, win, fonts, media, canvases };
}

function rig(t, initial = null, timeout = 5000) {
  const dom = documentMock();
  const draws = [], commits = [], errors = [], measurements = [];
  const beforeCleanup = [];
  let hidden = 0, active = 0, maxActive = 0;
  const runtime = {
    measure(config, code) { measurements.push({ config, code }); if (config.content[0] === 'oversize') throw codedError(code); },
    draw(config) {
      active++;
      maxActive = Math.max(maxActive, active);
      return new Promise((resolve, reject) => {
        let settled = false;
        draws.push({
          config,
          resolve(value = tile(config.content[0])) { if (settled) return; settled = true; active--; resolve(value); },
          reject(error = new Error('decode failed')) { if (settled) return; settled = true; active--; reject(error); },
        });
      });
    },
  };
  const controller = createWatermarkController(dom.doc, initial && normalizeWatermark(dom.doc, options(initial)), {
    commit(value) { commits.push(value); },
    hide() { hidden++; },
    error(operation, error) { errors.push({ operation, error }); },
  }, runtime, timeout);
  t.after(async () => {
    for (const cleanup of beforeCleanup) cleanup();
    controller.destroy();
    for (const draw of draws) draw.resolve();
    await turn();
    assert.equal(dom.fonts.count + dom.win.count + dom.media.reduce((sum, item) => sum + item.count, 0), 0, 'all font/DPR listeners released');
  });
  return { ...dom, runtime, controller, draws, commits, errors, measurements, beforeCleanup, get hidden() { return hidden; }, get maxActive() { return maxActive; } };
}

function facadeRig(t, watermark) {
  const r = rig(t);
  const createElement = r.doc.createElement.bind(r.doc);
  class Element {
    children = [];
    style = { setProperty(name, value) { this[name] = value; }, removeProperty(name) { delete this[name]; } };
    ownerDocument = r.doc;
    scrollWidth = 640;
    clientWidth = 640;
    clientHeight = 0;
    appendChild(child) { this.children.push(child); child.parent = this; return child; }
    replaceChildren(...children) { this.children = []; children.forEach(child => this.appendChild(child)); }
    attachShadow() {
      this.shadowRoot = { children: [], get childNodes() { return this.children; }, querySelectorAll: () => [], replaceChildren(...children) { this.children = children; } };
      return this.shadowRoot;
    }
    getBoundingClientRect() { return { width: 640, height: 0 }; }
    remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this); }
  }
  r.doc.createElement = tag => tag === 'div' ? new Element() : createElement(tag);
  const container = new Element();
  const calls = { opened: 0, rendered: 0, disposed: 0 };
  globalThis.__watermarkFacadeRuntime = r.runtime;
  globalThis.__watermarkFacadeAdapter = {
    async open(request) { calls.opened++; return { format: request.format, document: {}, stats: {}, durationMs: 1, warnings: [] }; },
    render(host) {
      calls.rendered++;
      host.shadowRoot.replaceChildren({});
      return { stats: {}, resize() {}, dispose() { calls.disposed++; } };
    },
    destroy() {},
  };
  const viewer = createOfficeViewer(container, { watermark });
  const events = [];
  viewer.on(event => events.push(event));
  r.beforeCleanup.push(() => viewer.destroy());
  return { ...r, viewer, events, container, calls, Element };
}

const wordSource = () => ({ bytes: formatBytes('docx'), format: 'docx' });

test('watermark normalization preserves line spaces, counts Unicode code points and copies arrays', () => {
  const { doc } = documentMock();
  const content = ['\n', '  张三  \r\n内部资料', '\n'];
  const gap = [0, 20];
  const config = normalizeWatermark(doc, { content, gap, color: '#aBc', opacity: 0 });
  content[1] = 'changed'; gap[0] = 100;
  assert.deepEqual(config, {
    content: ['  张三  ', '内部资料'], gap: [0, 20], color: '#aabbcc', opacity: 0,
    rotate: -25, fontSize: 14, fontFamily: 'sans-serif',
  });
  assert.equal(normalizeWatermark(doc, { content: '😀'.repeat(128) }).content[0].length, 256);
  assert.throws(() => normalizeWatermark(doc, { content: '😀'.repeat(129) }), errorCode('INVALID_WATERMARK'));
  assert.throws(() => normalizeWatermark(doc, { content: ['a'.repeat(128), 'b'.repeat(128), 'c'] }), errorCode('INVALID_WATERMARK'));
});

test('watermark rejects empty/non-text content and out-of-range style inputs', () => {
  const { doc } = documentMock();
  for (const value of [undefined, null, {}, options(''), options(' \n\t'), options([]), options([3]),
    options('a\nb\nc\nd\ne'), { content: 'x', rotate: Infinity }, { content: 'x', rotate: 181 },
    { content: 'x', opacity: -0.1 }, { content: 'x', fontSize: 73 }, { content: 'x', fontSize: 7 },
    { content: 'x', gap: [0] }, { content: 'x', gap: [0, 513] }, { content: 'x', gap: [0, NaN] },
    { content: 'x', gap: [undefined, undefined] }, { content: 'x', gap: new Array(2) }, options(new Array(2)),
    { content: 'x', color: 'red' }, { content: 'x', color: '#abcd' },
    { content: 'x', fontFamily: '' }, { content: 'x', fontFamily: 'url(https://example.test/font)' },
    { content: 'x', fontFamily: 'sans-serif; color:red' }, { content: 'x', fontFamily: 'x'.repeat(257) },
    ...['initial', 'inherit', 'unset', 'revert', 'revert-layer', 'var(--font)', 'env(font)'].map(fontFamily => ({ content: 'x', fontFamily }))]) {
    assert.throws(() => normalizeWatermark(doc, value), errorCode('INVALID_WATERMARK'));
  }
});

test('watermark preflight rejects bitmap budgets before allocating a large Canvas', () => {
  const { doc, win, canvases } = documentMock();
  win.devicePixelRatio = 3;
  doc.glyphWidth = 70;
  assert.throws(() => validateWatermark(doc, { content: '汉'.repeat(128), fontSize: 72 }), errorCode('INVALID_WATERMARK'));
  assert.equal(canvases.length, 1);
  assert.equal(canvases[0].width, 0);
  assert.equal(canvases[0].height, 0);
  win.devicePixelRatio = 2;
  doc.glyphWidth = 14;
  assert.throws(() => validateWatermark(doc, { content: '汉'.repeat(128), rotate: 45, gap: [0, 0] }), errorCode('INVALID_WATERMARK'), 'area budget applies even when both edges are below 4096');
  win.devicePixelRatio = 100;
  doc.glyphWidth = 5;
  assert.doesNotThrow(() => validateWatermark(doc, { content: '汉'.repeat(128), rotate: 0 }), 'DPR is clamped to three before budgeting');
  doc.createElement = tag => tag === 'span' ? { style: {} } : { getContext: () => null };
  assert.doesNotThrow(() => validateWatermark(doc, options('no Canvas yet')), 'missing Canvas is an asynchronous rendering failure');
});

test('disabled controllers allocate no graph or font/DPR listeners, and same-turn updates draw only the latest', async t => {
  const r = rig(t);
  await turn();
  assert.equal(r.draws.length, 0);
  assert.equal(r.canvases.length, 0);
  assert.equal(r.fonts.count + r.win.count + r.media.length, 0);
  const first = r.controller.set(options('never drawn'));
  const second = r.controller.set(options('latest'));
  await assert.rejects(first, errorCode('ABORTED'));
  await turn();
  assert.deepEqual(r.draws.map(draw => draw.config.content[0]), ['latest']);
  r.draws[0].resolve(); await second;
  assert.equal(r.controller.get().content[0], 'latest');
});

test('watermark commit is atomic and getters return independent normalized snapshots', async t => {
  const r = rig(t);
  const content = ['first'];
  const gap = [1, 2];
  const updating = r.controller.set({ content, gap, rotate: 20 });
  content[0] = 'mutated'; gap[0] = 400;
  assert.equal(r.controller.get(), null);
  await turn();
  assert.equal(r.draws[0].config.content[0], 'first');
  r.draws[0].resolve();
  await updating;
  const copy = r.controller.get();
  copy.content[0] = 'leak'; copy.gap[0] = 100;
  assert.deepEqual(r.controller.get().content, ['first']);
  assert.deepEqual(r.controller.get().gap, [1, 2]);
  const replacement = r.controller.set(options('second'));
  await turn();
  assert.equal(r.controller.get().content[0], 'first');
  assert.equal(r.commits.length, 1);
  r.draws[1].resolve();
  await replacement;
  assert.equal(r.controller.get().rotate, -25, 'replacement restores omitted defaults');
  assert.equal(r.controller.get().content[0], 'second');
});

test('invalid watermark requests reject asynchronously without superseding a valid pending request', async t => {
  const r = rig(t);
  const updating = r.controller.set(options('valid'));
  await turn();
  for (const value of [options(''), options('oversize')]) {
    let promise;
    assert.doesNotThrow(() => { promise = r.controller.set(value); });
    assert.ok(promise instanceof Promise);
    await assert.rejects(promise, errorCode('INVALID_WATERMARK'));
  }
  assert.equal(r.controller.hasPending(), true);
  assert.equal(r.errors.length, 0);
  r.draws[0].resolve();
  await updating;
  assert.equal(r.controller.get().content[0], 'valid');
});

test('superseded watermark promises abort immediately while decodes remain strictly serial', async t => {
  const r = rig(t);
  const first = r.controller.set(options('A'));
  await turn();
  const waiting = r.controller.wait();
  const second = r.controller.set(options('B'));
  const third = r.controller.set(options('C'));
  await assert.rejects(first, errorCode('ABORTED'));
  await assert.rejects(second, errorCode('ABORTED'));
  assert.equal(r.draws.length, 1, 'superseding a Promise does not release its decode slot');
  r.draws[0].resolve();
  await turn();
  assert.deepEqual(r.draws.map(draw => draw.config.content[0]), ['A', 'C']);
  assert.equal(r.commits.length, 0, 'stale decode cannot commit');
  r.draws[1].resolve();
  await Promise.all([third, waiting]);
  assert.equal(r.controller.get().content[0], 'C');
  assert.equal(r.maxActive, 1);
  assert.equal(r.errors.length, 0);
});

test('null, clear and destroy settle pending setters before late decoding finishes', async t => {
  for (const action of ['null', 'clear', 'destroy']) {
    const r = rig(t);
    const updating = r.controller.set(options(action));
    await turn();
    if (action === 'null') await r.controller.set(null);
    else r.controller[action]();
    await assert.rejects(updating, errorCode(action === 'destroy' ? 'DESTROYED' : 'ABORTED'));
    assert.equal(r.controller.get(), null);
    assert.ok(r.hidden >= 1);
    r.draws[0].resolve();
    await turn();
    assert.equal(r.commits.length, 0);
    assert.equal(r.errors.length, 0);
    if (action === 'destroy') {
      await assert.rejects(r.controller.set(options('late')), errorCode('DESTROYED'));
      await assert.rejects(r.controller.wait(), errorCode('DESTROYED'));
    }
  }
});

test('failed update retains the committed tile, and clear rolls pending work back to that configuration', async t => {
  const r = rig(t);
  const original = r.controller.set(options('applied'));
  await turn(); r.draws[0].resolve(); await original;
  const failed = r.controller.set(options('failed'));
  await turn(); r.draws[1].reject();
  await assert.rejects(failed, errorCode('WATERMARK_FAILED'));
  assert.equal(r.controller.get().content[0], 'applied');
  assert.equal(r.commits.length, 1);
  assert.equal(r.errors.length, 1);
  assert.equal(r.errors[0].operation, 'update');
  const pending = r.controller.set(options('discarded'));
  await turn(); r.controller.clear();
  await assert.rejects(pending, errorCode('ABORTED'));
  r.draws[2].resolve(); await turn();
  const reopening = r.controller.wait();
  await turn(); r.draws[3].resolve();
  await reopening;
  r.controller.mount();
  assert.equal(r.controller.get().content[0], 'applied');
  assert.equal(r.commits.at(-1).url, tile('applied').url);
  assert.equal(r.draws.length, 4, 'reopen revalidates the retained tile after listeners were removed');
});

test('reopen refreshes after font and DPR changes while clear had removed listeners', async t => {
  const r = rig(t);
  const environments = [];
  const draw = r.runtime.draw;
  r.runtime.draw = config => {
    environments.push({ dpr: r.win.devicePixelRatio, glyphWidth: r.doc.glyphWidth });
    return draw(config);
  };
  const first = r.controller.set(options('persisted'));
  await turn(); r.draws[0].resolve(); await first; await turn();
  r.controller.clear();
  assert.equal(r.fonts.count + r.win.count, 0);
  r.win.devicePixelRatio = 2;
  r.doc.glyphWidth = 16;
  r.fonts.dispatchEvent(new Event('loadingdone'));
  const reopening = r.controller.wait();
  await turn();
  assert.deepEqual(environments, [{ dpr: 1, glyphWidth: 8 }, { dpr: 2, glyphWidth: 16 }]);
  r.draws[1].resolve(); await reopening;
  assert.equal(r.controller.get().content[0], 'persisted');
  assert.equal(r.errors.length, 0);
});

test('a failed reopen refresh reports independently and retains the cached applied tile', async t => {
  const r = rig(t);
  const first = r.controller.set(options('persisted'));
  await turn(); r.draws[0].resolve(); await first; await turn();
  r.controller.clear();
  const reopening = r.controller.wait();
  await turn(); r.draws[1].reject();
  await reopening;
  r.controller.mount();
  assert.equal(r.controller.get().content[0], 'persisted');
  assert.equal(r.commits.at(-1).url, tile('persisted').url);
  assert.equal(r.errors.length, 1);
  assert.equal(r.errors[0].operation, 'refresh');
});

test('initial failure blocks every wait with the same error until an explicit recovery succeeds', async t => {
  const r = rig(t, 'initial');
  const opening = r.controller.wait();
  await turn(); r.draws[0].reject();
  let initialError;
  await assert.rejects(opening, error => { initialError = error; return error.code === 'WATERMARK_FAILED'; });
  assert.equal(r.errors[0].error, initialError);
  assert.equal(r.errors[0].operation, 'initialize');
  r.controller.clear();
  await assert.rejects(r.controller.wait(), error => error === initialError);
  assert.equal(r.draws.length, 1, 'clear does not retry or silently disable a failed constructor watermark');
  const recovery = r.controller.set(options('recovered'));
  await turn(); r.draws[1].resolve(); await recovery;
  await r.controller.wait();
  assert.equal(r.controller.get().content[0], 'recovered');
  await r.controller.set(null);
  await r.controller.wait();
});

test('clear during unfinished initialization preserves intent for the next open', async t => {
  const r = rig(t, 'initial');
  await turn();
  r.controller.clear();
  assert.equal(r.controller.get(), null);
  const reopening = r.controller.wait();
  assert.equal(r.draws.length, 1);
  r.draws[0].resolve(); await turn();
  assert.equal(r.commits.length, 0);
  assert.equal(r.draws[1].config.content[0], 'initial');
  r.draws[1].resolve(); await reopening;
  assert.equal(r.controller.get().content[0], 'initial');
});

test('explicit null unblocks failed initialization, while invalid updates preserve the initial failure', async t => {
  const r = rig(t, 'required');
  const opening = r.controller.wait();
  await turn(); r.draws[0].reject();
  let original;
  await assert.rejects(opening, error => { original = error; return error.code === 'WATERMARK_FAILED'; });
  await assert.rejects(r.controller.set(options('')), errorCode('INVALID_WATERMARK'));
  await assert.rejects(r.controller.wait(), error => error === original);
  await r.controller.set(null);
  await r.controller.wait();
  assert.equal(r.draws.length, 1);
  assert.equal(r.controller.get(), null);
});

test('explicit first-update failure does not leave a hidden open blocker', async t => {
  const r = rig(t);
  const updating = r.controller.set(options('optional'));
  await turn(); r.draws[0].reject();
  await assert.rejects(updating, errorCode('WATERMARK_FAILED'));
  await r.controller.wait();
  assert.equal(r.controller.get(), null);
});

test('timed-out decodes hold the slot, reject new work and never replay rejected requests', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const r = rig(t, null, 50);
  const updating = r.controller.set(options('slow'));
  await turn();
  t.mock.timers.tick(50);
  await assert.rejects(updating, errorCode('WATERMARK_FAILED'));
  await assert.rejects(r.controller.set(options('blocked')), errorCode('WATERMARK_FAILED'));
  assert.equal(r.draws.length, 1);
  r.draws[0].resolve(); await turn();
  assert.equal(r.commits.length, 0);
  assert.equal(r.draws.length, 1, 'no automatic retry after underlying decode finishes');
  const recovery = r.controller.set(options('retry'));
  await turn(); r.draws[1].resolve(); await recovery;
  assert.equal(r.controller.get().content[0], 'retry');
  assert.equal(r.maxActive, 1);
});

test('font and DPR refreshes coalesce, preserve old tiles on failure, and cannot overwrite a new identity', async t => {
  const r = rig(t);
  const first = r.controller.set(options('old-user'));
  await turn(); r.draws[0].resolve(); await first; await turn();
  r.fonts.dispatchEvent(new Event('loadingdone'));
  r.fonts.dispatchEvent(new Event('loadingdone'));
  await turn();
  assert.equal(r.draws.length, 2);
  const changed = r.controller.set(options('new-user'));
  r.draws[1].resolve(); await turn();
  assert.equal(r.commits.length, 1, 'old font redraw cannot replace the new user');
  r.draws[2].resolve(); await changed; await turn();
  r.win.devicePixelRatio = 2;
  r.win.dispatchEvent(new Event('resize'));
  r.media.at(-1).dispatchEvent(new Event('change'));
  await turn();
  assert.equal(r.draws.length, 4);
  r.draws[3].reject(); await turn();
  assert.equal(r.controller.get().content[0], 'new-user');
  assert.equal(r.errors.length, 1);
  assert.equal(r.errors[0].operation, 'refresh');
  r.win.dispatchEvent(new Event('resize')); await turn();
  assert.equal(r.draws.length, 4, 'ordinary resize never re-encodes the tile');
  await r.controller.set(null);
  r.fonts.dispatchEvent(new Event('loadingdone'));
  r.win.devicePixelRatio = 3; r.win.dispatchEvent(new Event('resize')); await turn();
  assert.equal(r.draws.length, 4, 'disabled viewer has no background work');
});

test('fonts and DPR changing during initialization are redrawn once after the initial tile commits', async t => {
  const r = rig(t, 'initial-user');
  const opening = r.controller.wait();
  await turn();
  assert.equal(r.fonts.count, 1, 'initialization observes font changes before its first decode finishes');
  r.fonts.dispatchEvent(new Event('loadingdone'));
  r.fonts.dispatchEvent(new Event('loadingdone'));
  r.win.devicePixelRatio = 2;
  r.win.dispatchEvent(new Event('resize'));
  assert.equal(r.draws.length, 1, 'changes cannot allocate another candidate while decoding');
  r.draws[0].resolve(); await opening; await turn();
  assert.deepEqual(r.draws.map(draw => draw.config.content[0]), ['initial-user', 'initial-user']);
  r.draws[1].resolve(); await turn();
  assert.equal(r.commits.length, 2);
  assert.equal(r.draws.length, 2, 'multiple font/DPR changes produce one catch-up draw');
  assert.equal(r.maxActive, 1);
});

test('font changes during superseded updates refresh only the latest committed identity', async t => {
  const r = rig(t);
  const original = r.controller.set(options('old'));
  await turn(); r.draws[0].resolve(); await original; await turn();
  const middle = r.controller.set(options('middle'));
  await turn();
  r.fonts.dispatchEvent(new Event('loadingdone'));
  const latest = r.controller.set(options('latest'));
  await assert.rejects(middle, errorCode('ABORTED'));
  r.draws[1].resolve(); await turn();
  assert.equal(r.draws[2].config.content[0], 'latest');
  r.fonts.dispatchEvent(new Event('loadingdone'));
  r.win.devicePixelRatio = 3; r.win.dispatchEvent(new Event('resize'));
  r.draws[2].resolve(); await latest; await turn();
  assert.deepEqual(r.draws.map(draw => draw.config.content[0]), ['old', 'middle', 'latest', 'latest']);
  assert.deepEqual(r.commits.map(committed => committed.url), [tile('old').url, tile('latest').url]);
  r.draws[3].resolve(); await turn();
  assert.equal(r.controller.get().content[0], 'latest');
  assert.equal(r.commits.at(-1).url, tile('latest').url);
  assert.equal(r.maxActive, 1);
});

test('clear and timeout discard dirty font refreshes instead of silently replaying them', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  for (const action of ['clear', 'timeout']) {
    const r = rig(t, null, 50);
    const original = r.controller.set(options('applied'));
    await turn(); r.draws[0].resolve(); await original; await turn();
    const updating = r.controller.set(options('pending'));
    await turn();
    r.fonts.dispatchEvent(new Event('loadingdone'));
    r.win.devicePixelRatio = 2; r.win.dispatchEvent(new Event('resize'));
    if (action === 'clear') r.controller.clear();
    else t.mock.timers.tick(50);
    await assert.rejects(updating, errorCode(action === 'clear' ? 'ABORTED' : 'WATERMARK_FAILED'));
    r.draws[1].resolve(); await turn(); await turn();
    assert.equal(r.draws.length, 2, `${action} does not replay its stale font request`);
    assert.equal(r.controller.get().content[0], 'applied');
    assert.equal(r.commits.length, 1);
    r.controller.destroy();
  }
});

test('facade rejects invalid constructor watermarks before attaching any owned DOM', t => {
  const r = facadeRig(t);
  const container = new r.Element();
  container.appendChild({ marker: 'caller' });
  assert.throws(() => createOfficeViewer(container, { watermark: options('') }), errorCode('INVALID_WATERMARK'));
  assert.deepEqual(container.children, [{ marker: 'caller', parent: container }]);
});

test('facade never mounts before the latest initial watermark is ready', async t => {
  const r = facadeRig(t, options('initial'));
  const opening = r.viewer.open(wordSource());
  await turn();
  assert.equal(r.calls.rendered, 0);
  assert.ok(r.events.some(event => event.type === 'statechange' && event.state === 'loading'));
  const replacement = r.viewer.setWatermark(options('latest'));
  r.draws[0].resolve(); await turn();
  assert.equal(r.calls.rendered, 0, 'an internal ABORTED is not a file cancellation or permission to mount');
  r.draws[1].resolve();
  const [, result] = await Promise.all([replacement, opening]);
  assert.equal(result.format, 'docx');
  assert.equal(r.viewer.getWatermark().content[0], 'latest');
  assert.equal(r.calls.rendered, 1);
  assert.deepEqual(r.events.filter(event => event.type === 'error'), []);
});

test('facade reuses the initial watermark error identity for open errors and recovers explicitly', async t => {
  const r = facadeRig(t, options('required'));
  const opening = r.viewer.open(wordSource());
  await turn(); r.draws[0].reject();
  let initialError;
  await assert.rejects(opening, error => { initialError = error; return error.code === 'WATERMARK_FAILED'; });
  assert.equal(r.calls.rendered, 0);
  assert.equal(r.events.find(event => event.type === 'watermarkerror').error, initialError);
  assert.equal(r.events.find(event => event.type === 'error').error, initialError);
  r.viewer.clear();
  await assert.rejects(r.viewer.open(wordSource()), error => error === initialError);
  await r.viewer.setWatermark(null);
  await r.viewer.open(wordSource());
  assert.equal(r.calls.rendered, 1);
});

test('facade update failures keep ready documents and zoom without reparsing', async t => {
  const r = facadeRig(t);
  await r.viewer.open(wordSource());
  r.viewer.setZoom(1.5);
  const updating = r.viewer.setWatermark(options('optional'));
  await turn(); r.draws[0].reject();
  await assert.rejects(updating, errorCode('WATERMARK_FAILED'));
  assert.equal(r.calls.opened, 1);
  assert.equal(r.calls.rendered, 1);
  assert.deepEqual(r.viewer.getZoom(), { mode: 'fixed', scale: 1.5 });
  assert.equal(r.events.filter(event => event.type === 'error').length, 0);
  assert.equal(r.events.filter(event => event.type === 'watermarkerror').length, 1);
  assert.equal(r.events.filter(event => event.type === 'statechange').at(-1).state, 'ready');
});

test('facade abort and destroy settle opens that are waiting for watermark decoding', async t => {
  for (const action of ['abort', 'clear', 'destroy']) {
    const r = facadeRig(t, options('required'));
    const signal = new AbortController();
    const opening = r.viewer.open({ ...wordSource(), signal: signal.signal });
    await turn();
    if (action === 'abort') signal.abort();
    else r.viewer[action]();
    await assert.rejects(opening, errorCode(action === 'destroy' ? 'DESTROYED' : 'ABORTED'));
    r.draws[0].resolve(); await turn();
    assert.equal(r.calls.rendered, 0);
    assert.equal(r.events.filter(event => event.type === 'error').length, 0);
    r.viewer.destroy();
  }
});
