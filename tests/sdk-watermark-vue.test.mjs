import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const tick = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const error = code => Object.assign(new Error(code), { code });

async function component(path, dependencies) {
  const file = await readFile(new URL(path, import.meta.url), 'utf8');
  const source = file.match(/<script>([\s\S]*?)<\/script>/)[1]
    .replace(/import[\s\S]*?from\s+['"][^'"]+['"];?/g, '')
    .replace('export default', 'return');
  return new Function(...Object.keys(dependencies), source)(...Object.values(dependencies));
}

function instance(options, props = {}) {
  const events = [];
  const vm = {
    ...props,
    $refs: {},
    $emit: (...event) => events.push(event),
    $nextTick: callback => Promise.resolve().then(callback),
  };
  for (const [name, prop] of Object.entries(options.props)) {
    if (!(name in vm)) vm[name] = typeof prop.default === 'function' && prop.type !== Function ? prop.default() : prop.default;
  }
  Object.assign(vm, options.data?.call(vm));
  for (const [name, method] of Object.entries(options.methods)) vm[name] = method.bind(vm);
  for (const [name, getter] of Object.entries(options.computed || {})) Object.defineProperty(vm, name, { get: getter.bind(vm) });
  options.created?.call(vm);
  return { vm, events };
}

test('Vue source and watermark changes wait for the latest identity; watermark-only updates do not reopen', async () => {
  const calls = [];
  let pending;
  const viewer = {
    clear() { calls.push('clear'); },
    on() { return () => {}; },
    async open(source) { calls.push(source.name); return { format: 'docx' }; },
    setWatermark(value) {
      pending?.reject(error('ABORTED'));
      pending = deferred();
      calls.push(value.content);
      return pending.promise;
    },
    getWatermark() { return null; },
  };
  const options = await component('../fixtures/vue2-vite/src/OfficePreview.vue', { createOfficeViewer: () => viewer });
  const { vm, events } = instance(options, { source: null });
  options.mounted.call(vm);
  await tick();
  calls.length = 0;
  vm.source = { name: 'new.docx' };
  vm.watermark = { content: 'A' };
  options.watch.source.call(vm);
  options.watch.watermark.call(vm);
  await tick();
  assert.deepEqual(calls, ['clear', 'A']);
  vm.watermark = { content: 'B' };
  options.watch.watermark.call(vm);
  await tick();
  pending.resolve();
  await tick();
  assert.deepEqual(calls, ['clear', 'A', 'B', 'new.docx']);
  vm.watermark = { content: 'C' };
  options.watch.watermark.call(vm);
  await tick();
  pending.resolve();
  await tick();
  assert.deepEqual(calls, ['clear', 'A', 'B', 'new.docx', 'C']);
  assert.equal(events.filter(([name]) => name === 'ready').length, 1);
  assert.equal(events.filter(([name]) => name === 'error').length, 0);
});

test('Vue rejects a new source with an invalid watermark and reports runtime failures separately', async () => {
  const opens = [];
  const viewer = {
    clear() {}, on() { return () => {}; },
    open(source) { opens.push(source.name); return Promise.resolve({}); },
    setWatermark() { return Promise.reject(error('INVALID_WATERMARK')); },
  };
  const options = await component('../fixtures/vue2-vite/src/OfficePreview.vue', { createOfficeViewer: () => viewer });
  const { vm, events } = instance(options);
  options.mounted.call(vm);
  await tick();
  vm.watermark = { content: '' };
  options.watch.watermark.call(vm);
  await tick();
  assert.equal(events.filter(([name]) => name === 'watermark-error').length, 1);
  assert.equal(events.filter(([name]) => name === 'error').length, 0);
  vm.source = { name: 'sensitive.docx' };
  options.watch.source.call(vm);
  await tick();
  assert.deepEqual(opens, []);
  assert.equal(events.filter(([name]) => name === 'error').length, 1);
});

test('Vue retries a failed runtime identity before opening a later source', async () => {
  let attempts = 0;
  const calls = [];
  const viewer = {
    clear() { calls.push('clear'); }, on() { return () => {}; },
    open(source) { calls.push(source.name); return Promise.resolve({}); },
    setWatermark(value) {
      calls.push(value.content);
      return ++attempts === 1 ? Promise.reject(error('WATERMARK_FAILED')) : Promise.resolve();
    },
  };
  const options = await component('../fixtures/vue2-vite/src/OfficePreview.vue', { createOfficeViewer: () => viewer });
  const { vm } = instance(options);
  options.mounted.call(vm);
  await tick();
  vm.watermark = { content: 'current user' };
  options.watch.watermark.call(vm);
  await tick();
  vm.source = { name: 'later.docx' };
  options.watch.source.call(vm);
  await tick();
  assert.deepEqual(calls, ['current user', 'clear', 'current user', 'later.docx']);
});

async function dialogHarness() {
  const sessions = [];
  const options = await component('../docs/examples/attachment-preview/AttachmentPreviewDialog.vue', {
    createPreviewSession(host, config) {
      const ready = deferred();
      const watermark = deferred();
      const session = {
        config, ready: ready.promise, watermarkReady: watermark.promise,
        finish: result => { watermark.resolve(); ready.resolve(result); },
        finishWatermark: watermark.resolve, failDocument: ready.reject, pending: null,
        applied: config.watermark, updates: 0,
        setWatermark(value) {
          session.updates++;
          session.pending = deferred();
          return session.pending.promise.then(() => { session.applied = value; });
        },
        getWatermark() { return session.applied; },
        getZoom() { return { scale: 1.5 }; },
        remove() {}, resize() {},
      };
      sessions.push(session);
      return session;
    },
    fetchAttachment() {}, getFileName: name => name,
    isCancelled: value => ['ABORTED', 'DESTROYED'].includes(value?.code), saveBlob() {},
  });
  const { vm, events } = instance(options, { visible: true, fileUrl: '/file.docx', fileName: 'file.docx' });
  vm._resizeObserver = { observe() {}, disconnect() {} };
  vm.$refs.officeHost = { clientWidth: 800 };
  return { options, vm, events, sessions };
}

test('attachment lazy setters initialize once with the latest watermark and close rejects pending work', async () => {
  const { vm, sessions } = await dialogHarness();
  const a = vm.setWatermark({ content: 'A' });
  const rejectedA = assert.rejects(a, { code: 'ABORTED' });
  const bValue = { content: 'B' };
  const b = vm.setWatermark(bValue);
  let settled = false;
  void b.then(() => { settled = true; });
  const opening = vm.queuePreview();
  await tick();
  assert.equal(sessions.length, 1);
  assert.equal(sessions[0].config.watermark, bValue);
  assert.equal(sessions[0].updates, 0);
  assert.equal(settled, false);
  sessions[0].finishWatermark();
  await b;
  assert.equal(settled, true, 'Watermark commit must not wait for the document');
  sessions[0].finish({ format: 'docx' });
  await opening;
  await rejectedA;
  const c = vm.setWatermark({ content: 'C' });
  const rejectedC = assert.rejects(c, { code: 'ABORTED' });
  vm.visible = false;
  vm.cleanup();
  await rejectedC;
  await assert.rejects(vm.setWatermark(null), { code: 'ABORTED' });
});

test('attachment runtime updates preserve the document and zoom; prop and service intent apply once', async () => {
  const { options, vm, events, sessions } = await dialogHarness();
  const opening = vm.queuePreview();
  await tick();
  const session = sessions[0];
  session.finish({ format: 'docx' });
  await opening;
  const value = { content: 'new user' };
  const update = vm.setWatermark(value);
  vm.watermark = value;
  options.watch.watermark.call(vm, value);
  assert.equal(session.updates, 1);
  assert.equal(sessions.length, 1);
  session.pending.resolve();
  await update;
  assert.equal(vm.getWatermark(), value);
  assert.equal(vm.zoomScale, 1.5);
  const failure = vm.setWatermark({ content: 'bad' });
  session.pending.reject(error('WATERMARK_FAILED'));
  await assert.rejects(failure, { code: 'WATERMARK_FAILED' });
  assert.equal(vm.getWatermark(), value);
  assert.equal(vm.canZoom, true);
  assert.equal(vm.errorMessage, '');
  assert.equal(events.filter(([name]) => name === 'watermark-error').length, 1);
});

test('attachment source replacement carries the newest pending identity into its new session', async () => {
  const { options, vm, sessions } = await dialogHarness();
  let opening = vm.queuePreview();
  await tick();
  const oldSession = sessions[0];
  oldSession.finish({ format: 'docx' });
  await opening;
  const value = { content: 'new attachment user' };
  const update = vm.setWatermark(value);
  let committed = false;
  void update.then(() => { committed = true; });
  vm.fileUrl = '/another.docx';
  opening = options.watch.fileUrl.call(vm);
  // The watcher invokes queuePreview synchronously; await the session creation.
  await tick();
  assert.equal(sessions.length, 2);
  assert.equal(sessions[1].config.watermark, value);
  oldSession.pending.resolve();
  await tick();
  assert.equal(committed, false, 'Outgoing session cannot commit the new request');
  sessions[1].finishWatermark();
  await update;
  assert.equal(committed, true);
  sessions[1].finish({ format: 'docx' });
  await opening;
});

test('session watermark barriers finish before download and follow updates during download', async () => {
  const file = await readFile(new URL('../docs/examples/attachment-preview/preview-session.js', import.meta.url), 'utf8');
  const source = file.replace(/import[^;]+;/g, '').replaceAll('export function', 'function').replaceAll('export async function', 'async function');
  const requests = [];
  const opens = [];
  const viewer = {
    on() { return () => {}; },
    setWatermark(value) {
      const previous = requests.at(-1);
      if (previous && !previous.done) previous.reject(error('ABORTED'));
      const task = { ...deferred(), value, done: false };
      requests.push(task);
      return task.promise;
    },
    open(request) { opens.push(request); return Promise.resolve({ format: 'docx' }); },
    setZoom() {}, destroy() {}, getWatermark() { return requests.at(-1)?.value; },
  };
  const { createPreviewSession } = new Function('createOfficeViewer', `${source}; return { createPreviewSession };`)(() => viewer);
  const download = deferred();
  const session = createPreviewSession({}, { fileUrl: '/file', fileName: 'file.docx', watermark: { content: 'A' }, fetchFile: () => download.promise });
  requests[0].done = true;
  requests[0].resolve();
  await session.watermarkReady;
  assert.equal(opens.length, 0, 'Initial watermarks commit before the file arrives');
  const update = session.setWatermark({ content: 'B' });
  download.resolve(new ArrayBuffer(1));
  await tick();
  assert.equal(opens.length, 0, 'Download completion cannot bypass an in-flight watermark');
  requests[1].done = true;
  requests[1].resolve();
  await update;
  await session.ready;
  assert.equal(opens.length, 1);
  assert.equal(session.getWatermark().content, 'B');
});

test('attachment service setters wait for commit, roll back failed intent, and reject on disposal', async () => {
  let child;
  class Vue {
    constructor(options) {
      this.options = options;
      this.$refs = {};
      this.$nextTick = callback => Promise.resolve().then(callback);
      this.$emit = () => {};
      Object.assign(this, options.data());
      for (const [key, method] of Object.entries(options.methods)) this[key] = method.bind(this);
    }
    $mount() {
      this.$refs.preview = child;
      this.$el = { remove() {} };
    }
    $destroy() { this.options.beforeDestroy.call(this); this._isDestroyed = true; }
  }
  const originalDocument = globalThis.document;
  globalThis.document = { body: { appendChild() {} } };
  try {
    const file = await readFile(new URL('../docs/examples/attachment-preview/service.js', import.meta.url), 'utf8');
    const source = file.replace(/import[^;]+;/g, '').replaceAll('export function', 'function');
    const service = new Function('Vue', 'AttachmentPreviewDialog', `${source}; return { openAttachmentPreview, closeAttachmentPreview };`)(Vue, {});
    const previous = { content: ['old user'] };
    child = {
      pending: null, intent: null,
      setWatermark() { this.pending = deferred(); return this.pending.promise; },
      getWatermark() { return previous; },
      syncWatermarkIntent(value) { this.intent = value; },
    };
    const dialog = service.openAttachmentPreview();
    const value = { content: 'new user' };
    const update = dialog.setWatermark(value);
    let done = false;
    void update.then(() => { done = true; });
    await tick();
    assert.equal(done, false);
    assert.equal(dialog.desiredWatermark, value);
    child.pending.resolve();
    await update;
    const failure = dialog.setWatermark({ content: '' });
    child.pending.reject(error('INVALID_WATERMARK'));
    await assert.rejects(failure, { code: 'INVALID_WATERMARK' });
    assert.equal(dialog.desiredWatermark, previous);
    assert.equal(child.intent, previous);
    const closing = dialog.setWatermark(value);
    const rejected = assert.rejects(closing, { code: 'DESTROYED' });
    service.closeAttachmentPreview();
    await rejected;
    await assert.rejects(dialog.setWatermark(null), { code: 'DESTROYED' });
  } finally {
    globalThis.document = originalDocument;
  }
});
