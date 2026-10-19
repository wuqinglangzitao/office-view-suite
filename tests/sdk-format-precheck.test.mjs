import test from 'node:test';
import assert from 'node:assert/strict';
import { detectOfficeFormat, resolveOfficeFormat, createWorkerAdapter } from '../packages/viewer-core/dist/index.js';
import { formatBytes, zipDirectory } from './sdk-format-fixtures.mjs';

test('content wins over misleading extensions; explicit formats must match the format family', () => {
  assert.equal(resolveOfficeFormat(formatBytes('xlsx'), { name: 'mistake.pdf' }), 'xlsx');
  assert.equal(resolveOfficeFormat(formatBytes('pdf'), { name: 'unknown.bin' }), 'pdf');
  assert.throws(() => resolveOfficeFormat(formatBytes('xlsx'), { format: 'pdf' }), { code: 'INVALID_SOURCE' });
  // Macro-enabled OOXML packages may contain no VBA project.
  assert.equal(resolveOfficeFormat(formatBytes('xlsx'), { format: 'xlsm' }), 'xlsm');
  assert.equal(resolveOfficeFormat(formatBytes('docx'), { name: 'empty-macro.docm' }), 'docm');
  assert.equal(resolveOfficeFormat(formatBytes('xlsm'), { name: 'renamed.xlsx' }), 'xlsm');
});

test('ZIP64 directory offsets are resolved and known container damage is rejected', () => {
  assert.equal(detectOfficeFormat(zipDirectory(['xl/workbook.xml'], { zip64: true })), 'xlsx');
  const broken = zipDirectory(['xl/workbook.xml']);
  broken.writeUInt32LE(0xffffffff, broken.length - 22 + 16);
  for (const bytes of [broken, Buffer.from('PK\x03\x04cut'), zipDirectory(['other.xml']), Buffer.from('not pdf')]) {
    assert.throws(() => resolveOfficeFormat(bytes, { name: 'file.xlsx' }), { code: 'INVALID_SOURCE' });
  }
  // The precheck is not a PDF/OOXML parser. Valid signatures with damaged
  // contents reach the engine, which owns the eventual PARSE_FAILED error.
  assert.equal(resolveOfficeFormat(Buffer.from('%PDF-1.7\nbroken objects'), {}), 'pdf');
});

test('Office container signatures take priority over incidental PDF text', () => {
  const zip = zipDirectory(['word/document.xml', 'word/media/%PDF-image.png']);
  assert.equal(resolveOfficeFormat(zip), 'docx');
  const doc = formatBytes('doc');
  doc.set(Buffer.from('%PDF-1.7'), 32);
  assert.equal(resolveOfficeFormat(doc), 'doc');
  assert.throws(() => resolveOfficeFormat(Buffer.from('PK\x03\x04%PDF-1.7\nbroken zip')), { code: 'INVALID_SOURCE' });
  assert.equal(resolveOfficeFormat(Buffer.from('vendor prefix\n%PDF-1.7\n')), 'pdf');
});

test('standalone adapters check content before Worker creation', async () => {
  let workers = 0;
  const adapter = createWorkerAdapter({ format: 'xlsx', formats: ['xlsx', 'xlsm'], createWorker() { workers++; throw new Error('unexpected'); } });
  await assert.rejects(adapter.open({ bytes: formatBytes('pdf'), name: 'bad.xlsx' }), { code: 'UNSUPPORTED_FORMAT' });
  await assert.rejects(adapter.open({ bytes: Buffer.from('garbage'), format: 'xlsx' }), { code: 'INVALID_SOURCE' });
  assert.equal(workers, 0);
  adapter.destroy();
});
