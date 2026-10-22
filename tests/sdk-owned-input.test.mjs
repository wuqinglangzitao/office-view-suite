import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import test from 'node:test';
import { formatBytes } from './sdk-format-fixtures.mjs';

const require = createRequire(new URL('../packages/viewer-core/package.json', import.meta.url));
const ts = require('typescript');
const compile = source => ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const asModule = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const coreUrl = asModule(compile(await readFile(new URL('../packages/viewer-core/src/index.ts', import.meta.url), 'utf8')));
const core = await import(coreUrl);
let facade = compile(await readFile(new URL('../packages/viewer/src/index.ts', import.meta.url), 'utf8'))
  .replaceAll("'@office-view-suite/viewer-core'", JSON.stringify(coreUrl));
for (const module of ['watermark', 'watermark-layout']) {
  const source = compile(await readFile(new URL(`../packages/viewer/src/${module}.ts`, import.meta.url), 'utf8'))
    .replaceAll("'@office-view-suite/viewer-core'", JSON.stringify(coreUrl));
  facade = facade.replaceAll(`'./${module}'`, JSON.stringify(asModule(source)));
}
for (const kind of ['word', 'excel', 'ppt', 'pdf']) {
  const name = kind[0].toUpperCase() + kind.slice(1);
  facade = facade.replaceAll(`'@office-view-suite/viewer-${kind}'`, JSON.stringify(asModule(
    `export function create${name}Adapter() { return globalThis.__ownedInputAdapterFactory(); }`,
  )));
}
const { createOfficeViewer } = await import(asModule(facade));
const tick = () => new Promise(resolve => setImmediate(resolve));

class Element {
  children = [];
  style = {};
  ownerDocument = { createElement: () => new Element() };
  appendChild(child) { child.parent = this; this.children.push(child); }
  replaceChildren(...children) { this.children = []; for (const child of children) this.appendChild(child); }
  attachShadow() { return this.shadowRoot = { childNodes: [], replaceChildren(...children) { this.childNodes = children; } }; }
  getBoundingClientRect() { return { width: 600 }; }
  remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
}

function setup() {
  const messages = [];
  const adapter = core.createWorkerAdapter({
    format: 'docx', formats: ['doc', 'docx', 'xlsx', 'pptx', 'pdf'],
    createWorker() {
      return {
        postMessage(message, transfer) {
          messages.push(structuredClone(message, { transfer }));
          queueMicrotask(() => this.onmessage?.({ data: { id: message.id, ok: true, document: {}, durationMs: 1 } }));
        },
        terminate() {},
      };
    },
  });
  globalThis.__ownedInputAdapterFactory = () => ({
    async open(request) { return { ...await adapter.open(request), format: request.format, stats: {} }; },
    render() { return { stats: {}, dispose() {} }; },
    destroy: adapter.destroy,
  });
  return { adapter, messages };
}

test('facade hands off one copy and one OLE inspection while preserving caller bytes', async () => {
  const { messages } = setup();
  const viewer = createOfficeViewer(new Element());
  const bytes = new Uint8Array(formatBytes('doc')).buffer;
  const expected = [...new Uint8Array(bytes)];
  const slice = ArrayBuffer.prototype.slice;
  const decode = TextDecoder.prototype.decode;
  let copies = 0;
  let inspections = 0;
  ArrayBuffer.prototype.slice = function (...args) { copies++; return slice.apply(this, args); };
  TextDecoder.prototype.decode = function (...args) {
    if (this.encoding === 'utf-16le') inspections++;
    return decode.apply(this, args);
  };
  try {
    assert.equal((await viewer.open({ bytes, name: 'document.doc' })).format, 'doc');
    assert.equal(copies, 1, 'public open must copy the caller buffer only once');
    assert.equal(inspections, 1, 'adapter must reuse the facade inspection');
    assert.deepEqual([...new Uint8Array(bytes)], expected);
    assert.deepEqual([...new Uint8Array(messages[0].bytes)], expected);
  } finally {
    ArrayBuffer.prototype.slice = slice;
    TextDecoder.prototype.decode = decode;
    viewer.destroy();
  }
});

test('direct adapter input still copies subarrays and ignores forged validation fields', async () => {
  const { adapter, messages } = setup();
  try {
    const expected = formatBytes('docx');
    const padded = new Uint8Array(expected.length + 2);
    padded.set(expected, 1);
    const source = padded.subarray(1, -1);
    await adapter.open({ bytes: source, format: 'docx', owned: true, validated: true });
    assert.deepEqual([...new Uint8Array(messages[0].bytes)], [...expected]);
    assert.deepEqual([...source], [...expected]);
    await assert.rejects(adapter.open({ bytes: new Uint8Array([1, 2, 3]), format: 'docx', owned: true, validated: true }), { code: 'INVALID_SOURCE' });
    assert.equal(messages.length, 1);
  } finally { adapter.destroy(); }
});

test('prepared inputs are accepted once by identity, never by copied request fields', async () => {
  const caller = new Uint8Array(formatBytes('docx')).buffer;
  const request = await core.prepareOfficeRequest({ bytes: caller, name: 'document.docx' });
  assert.notEqual(request.bytes, caller);
  const forged = await core.takeOfficeInput({ ...request });
  assert.notEqual(forged.bytes, request.bytes, 'spreading a request must not transfer its ownership');
  const first = await core.takeOfficeInput(request);
  assert.equal(first.bytes, request.bytes);
  assert.equal(first.format, 'docx');
  const repeated = await core.takeOfficeInput(request);
  assert.notEqual(repeated.bytes, first.bytes, 'the internal ownership grant is single-use');
  assert.deepEqual(new Uint8Array(repeated.bytes), new Uint8Array(caller));
});

test('prepared input rejects incompatible adapters and pre-aborted requests', async () => {
  const caller = new Uint8Array(formatBytes('xlsx')).buffer;
  const request = await core.prepareOfficeRequest({ bytes: caller });
  let workers = 0;
  const adapter = core.createWorkerAdapter({ format: 'docx', createWorker() { workers++; throw new Error('must not start'); } });
  try {
    await assert.rejects(adapter.open(request), { code: 'UNSUPPORTED_FORMAT' });
    const controller = new AbortController();
    const aborted = await core.prepareOfficeRequest({ bytes: caller, signal: controller.signal });
    controller.abort();
    await assert.rejects(adapter.open(aborted), { code: 'ABORTED' });
    assert.equal(workers, 0);
    assert.ok(caller.byteLength > 0);
  } finally { adapter.destroy(); }
});

test('facade aborts a pending input read without initializing an adapter', async () => {
  const { messages } = setup();
  let finish;
  class SlowBlob extends Blob { arrayBuffer() { return new Promise(resolve => { finish = resolve; }); } }
  const viewer = createOfficeViewer(new Element());
  try {
    const opening = viewer.open({ bytes: new SlowBlob(['pending']), format: 'docx' });
    const rejection = assert.rejects(opening, { code: 'ABORTED' });
    viewer.clear();
    await rejection;
    finish(new Uint8Array(formatBytes('docx')).buffer);
    await tick();
    assert.equal(messages.length, 0);
  } finally { viewer.destroy(); }
});

test('obsolete Blob reads skip format inspection after abort, replacement, or destroy', async () => {
  const decode = TextDecoder.prototype.decode;
  let inspections = 0;
  TextDecoder.prototype.decode = function (...args) {
    if (this.encoding === 'utf-16le') inspections++;
    return decode.apply(this, args);
  };
  try {
    for (const action of ['abort', 'replace', 'destroy']) {
      const { adapter, messages } = setup();
      let finish;
      class SlowBlob extends Blob { arrayBuffer() { return new Promise(resolve => { finish = resolve; }); } }
      const controller = new AbortController();
      const opening = adapter.open({ bytes: new SlowBlob(['pending']), format: 'doc', signal: controller.signal });
      const rejection = assert.rejects(opening, { code: action === 'destroy' ? 'DESTROYED' : 'ABORTED' });
      if (action === 'abort') controller.abort();
      if (action === 'destroy') adapter.destroy();
      if (action === 'replace') await adapter.open({ bytes: formatBytes('docx') });
      await rejection;
      finish(new Uint8Array(formatBytes('doc')).buffer);
      await tick();
      assert.equal(inspections, 0, `obsolete ${action} request must not decode the file`);
      assert.equal(messages.length, action === 'replace' ? 1 : 0);
      adapter.destroy();
    }
  } finally { TextDecoder.prototype.decode = decode; }
});
