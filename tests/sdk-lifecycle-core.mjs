import assert from 'node:assert/strict';
import test from 'node:test';
import {
  copyOfficeBytes,
  createWorkerAdapter,
  detectOfficeFormat,
} from '../packages/viewer-core/dist/index.js';

const turn = () => new Promise(resolve => setImmediate(resolve));
const rejectedWith = code => error => error.code === code;
const input = () => ({ format: 'docx', bytes: new Uint8Array([1, 2, 3]) });

class FakeWorker {
  messages = [];
  terminated = false;
  postMessage(message, transfer) {
    this.messages.push(structuredClone(message, { transfer }));
  }
  terminate() { this.terminated = true; }
  reply(document = { text: 'parsed' }) {
    this.onmessage?.({ data: { id: this.messages.at(-1).id, ok: true, document, durationMs: 2, warnings: ['font fallback'] } });
  }
}

function setup(extra = {}) {
  const workers = [];
  const adapter = createWorkerAdapter({
    format: 'docx', formats: ['doc', 'docx', 'docm'],
    createWorker() { const worker = new FakeWorker(); workers.push(worker); return worker; },
    ...extra,
  });
  return { adapter, workers };
}

test('caller buffers remain usable after Worker transfer, including subarray bounds', async () => {
  for (const source of [new Uint8Array([9, 1, 2, 8]).subarray(1, 3), new Uint8Array([1, 2]).buffer, new Blob([new Uint8Array([1, 2])])]) {
    const { adapter, workers } = setup({ workerData: { wasmUrl: '/office/word/parser.wasm' } });
    const opened = adapter.open({ bytes: source, format: 'docx' });
    await turn();
    assert.deepEqual([...new Uint8Array(workers[0].messages[0].bytes)], [1, 2]);
    assert.equal(workers[0].messages[0].wasmUrl, '/office/word/parser.wasm');
    assert.deepEqual([...new Uint8Array(await copyOfficeBytes(source))], [1, 2]);
    workers[0].reply();
    assert.deepEqual((await opened).warnings, [{ code: 'DOCUMENT_WARNING', message: 'font fallback' }]);
    adapter.destroy();
  }
});

test('abort terminates synchronous parsing; the next request recreates its Worker', async () => {
  const { adapter, workers } = setup();
  const abort = new AbortController();
  const first = adapter.open({ ...input(), signal: abort.signal });
  const rejected = assert.rejects(first, rejectedWith('ABORTED'));
  await turn();
  const lateMessage = workers[0].onmessage;
  abort.abort();
  await rejected;
  assert.equal(workers[0].terminated, true);
  const second = adapter.open(input());
  await turn();
  lateMessage({ data: { id: 1, ok: true, document: { stale: true }, durationMs: 0 } });
  workers[1].reply({ fresh: true });
  assert.deepEqual((await second).document, { fresh: true });
  adapter.destroy();
});

test('replacement cancels previous work and reuses a successful idle Worker', async () => {
  const { adapter, workers } = setup();
  const first = adapter.open(input());
  const rejected = assert.rejects(first, rejectedWith('ABORTED'));
  await turn();
  const second = adapter.open(input());
  await rejected;
  await turn();
  assert.equal(workers[0].terminated, true);
  workers[1].reply();
  await second;
  const third = adapter.open(input());
  await turn();
  assert.equal(workers.length, 2);
  workers[1].reply();
  await third;
  adapter.destroy();
});

test('pre-aborted and invalid input never initialize a Worker', async () => {
  const { adapter, workers } = setup();
  await assert.rejects(adapter.open({ ...input(), signal: AbortSignal.abort() }), rejectedWith('ABORTED'));
  await assert.rejects(adapter.open({ bytes: 'invalid', format: 'docx' }), rejectedWith('INVALID_SOURCE'));
  await assert.rejects(adapter.open({ bytes: new ArrayBuffer(0), format: 'docx' }), rejectedWith('INVALID_SOURCE'));
  await assert.rejects(adapter.open({ ...input(), format: 'pdf' }), rejectedWith('UNSUPPORTED_FORMAT'));
  assert.equal(workers.length, 0);
  adapter.destroy();
});

test('abort or destroy settles promptly while a Blob read is still pending', async () => {
  for (const action of ['abort', 'destroy']) {
    let finishRead;
    class SlowBlob extends Blob { arrayBuffer() { return new Promise(resolve => { finishRead = resolve; }); } }
    const { adapter, workers } = setup();
    const abort = new AbortController();
    const opening = adapter.open({ bytes: new SlowBlob(['data']), format: 'docx', signal: abort.signal });
    const rejected = assert.rejects(opening, rejectedWith(action === 'abort' ? 'ABORTED' : 'DESTROYED'));
    if (action === 'abort') abort.abort(); else adapter.destroy();
    await rejected;
    finishRead(new Uint8Array([1, 2]).buffer);
    await turn();
    assert.equal(workers.length, 0);
    adapter.destroy();
    await assert.rejects(adapter.open(input()), rejectedWith('DESTROYED'));
  }
});

test('Worker construction, load, and message errors allow a clean retry', async () => {
  for (const failure of ['factory', 'error', 'messageerror']) {
    const workers = [];
    let attempts = 0;
    const { adapter } = setup({ createWorker() {
      if (++attempts === 1 && failure === 'factory') throw new Error('CSP blocked');
      const worker = new FakeWorker(); workers.push(worker); return worker;
    } });
    const first = adapter.open(input());
    const rejected = assert.rejects(first, rejectedWith(failure === 'messageerror' ? 'PARSE_FAILED' : 'ASSET_LOAD_FAILED'));
    await turn();
    if (failure === 'error') workers[0].onerror({ message: '404' });
    if (failure === 'messageerror') workers[0].onmessageerror({});
    await rejected;
    const second = adapter.open(input());
    await turn();
    workers.at(-1).reply();
    await second;
    adapter.destroy();
  }
});

function zipDirectory(names) {
  const records = names.map(name => {
    const nameBytes = Buffer.from(name);
    const record = Buffer.alloc(46 + nameBytes.length);
    record.writeUInt32LE(0x02014b50);
    record.writeUInt16LE(nameBytes.length, 28);
    nameBytes.copy(record, 46);
    return record;
  });
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(names.length, 10);
  end.writeUInt32LE(records.reduce((total, record) => total + record.length, 0), 12);
  return Buffer.concat([...records, end]);
}

test('format detection reads OOXML ZIP directory names and PDF signatures without a file name', async () => {
  for (const [names, format] of [
    [['word/document.xml'], 'docx'],
    [['word/document.xml', 'word/vbaProject.bin'], 'docm'],
    [['xl/workbook.xml'], 'xlsx'],
    [['xl/workbook.xml', 'xl/vbaProject.bin'], 'xlsm'],
    [['ppt/presentation.xml'], 'pptx'],
  ]) assert.equal(detectOfficeFormat(zipDirectory(names)), format);
  assert.equal(detectOfficeFormat(Buffer.from('%PDF-1.7\n')), 'pdf');
  assert.equal(detectOfficeFormat(zipDirectory(['other/file.xml'])), undefined);
  assert.equal(detectOfficeFormat(new Uint8Array([1, 2])), undefined);
  const { adapter, workers } = setup();
  const opening = adapter.open({ bytes: zipDirectory(['word/document.xml']) });
  await turn();
  assert.equal(workers[0].messages[0].name, 'document.docx');
  workers[0].reply();
  await opening;
  adapter.destroy();
});
