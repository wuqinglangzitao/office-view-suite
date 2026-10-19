import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyPdfiumNotices } from '../scripts/verify-pdfium-notices.mjs';

const source = fileURLToPath(new URL('../third_party/pdfium/', import.meta.url));
const distribution = fileURLToPath(new URL('../packages/viewer-pdf/dist/', import.meta.url));
const hash = contents => createHash('sha256').update(contents).digest('hex');

async function fixture(t) {
  const directory = await mkdtemp(resolve(tmpdir(), 'pdfium-notices-test-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const files = { 'pdfium.std.js': 'fixture javascript', 'pdfium.std.wasm': 'fixture wasm', 'licenses/PDFium/LICENSE': 'fixture license text' };
  for (const [path, contents] of Object.entries(files)) {
    await mkdir(dirname(resolve(directory, path)), { recursive: true });
    await writeFile(resolve(directory, path), contents);
  }
  for (const path of ['SOURCE.md', 'NOTICE.md', 'BUILD_EVIDENCE.md']) await writeFile(resolve(directory, path), 'Test fixture only.');
  const manifest = {
    schemaVersion: 1,
    artifact: {
      release: 'test-fixture', archive: { url: 'https://example.com/wasm.zip', sha256: hash('fixture archive') },
      files: ['pdfium.std.js', 'pdfium.std.wasm'].map(path => ({ path, archivePath: `release/node/${path}`, sha256: hash(files[path]) })),
    },
    components: [{ name: 'PDFium', version: 'test', source: 'https://example.com/source', license: 'BSD-3-Clause', licenseFiles: ['licenses/PDFium/LICENSE'] }],
    licenseFiles: [{ path: 'licenses/PDFium/LICENSE', sha256: hash(files['licenses/PDFium/LICENSE']), sourceUrl: 'https://example.com/LICENSE' }],
  };
  const save = () => writeFile(resolve(directory, 'provenance.json'), JSON.stringify(manifest));
  await save();
  return { directory, manifest, save };
}

test('PDFium: checked-in runtime and license evidence verify offline', async () => {
  await verifyPdfiumNotices(source);
});

test('PDFium: valid isolated fixture verifies', async t => {
  const { directory, manifest } = await fixture(t);
  assert.deepEqual(await verifyPdfiumNotices(directory), manifest);
});

for (const path of ['pdfium.std.js', 'pdfium.std.wasm', 'licenses/PDFium/LICENSE']) {
  test(`PDFium: rejects changed ${path}`, async t => {
    const { directory } = await fixture(t);
    await writeFile(resolve(directory, path), 'modified');
    await assert.rejects(verifyPdfiumNotices(directory), /SHA-256 mismatch/);
  });
}

test('PDFium: rejects a missing license file', async t => {
  const { directory } = await fixture(t);
  await rm(resolve(directory, 'licenses/PDFium/LICENSE'));
  await assert.rejects(verifyPdfiumNotices(directory), /ENOENT/);
});

test('PDFium: rejects empty license text even with its matching hash', async t => {
  const { directory, manifest, save } = await fixture(t);
  await writeFile(resolve(directory, 'licenses/PDFium/LICENSE'), '');
  manifest.licenseFiles[0].sha256 = hash('');
  await save();
  await assert.rejects(verifyPdfiumNotices(directory), /Empty PDFium file/);
});

test('PDFium: rejects a component with a dangling license reference', async t => {
  const { directory, manifest, save } = await fixture(t);
  manifest.components[0].licenseFiles = ['licenses/missing.txt'];
  await save();
  await assert.rejects(verifyPdfiumNotices(directory), /Unknown license reference/);
});

test('PDFium: rejects a duplicated binary instead of the required pair', async t => {
  const { directory, manifest, save } = await fixture(t);
  manifest.artifact.files[1] = manifest.artifact.files[0];
  await save();
  await assert.rejects(verifyPdfiumNotices(directory), /duplicate PDFium binary path/);
});

for (const path of ['../outside.txt', '/outside.txt', 'licenses/../../outside.txt', 'licenses\\outside.txt']) {
  test(`PDFium: rejects unsafe license path ${path}`, async t => {
    const { directory, manifest, save } = await fixture(t);
    manifest.licenseFiles[0].path = path;
    await save();
    await assert.rejects(verifyPdfiumNotices(directory), /Unsafe relative file path/);
  });
}

test('PDFium: rejects a license symlink escaping the evidence directory', async t => {
  const { directory } = await fixture(t);
  await rm(resolve(directory, 'licenses/PDFium/LICENSE'));
  await symlink(fileURLToPath(import.meta.url), resolve(directory, 'licenses/PDFium/LICENSE'));
  await assert.rejects(verifyPdfiumNotices(directory), /File escapes PDFium directory/);
});

test('PDFium: rejects a missing build evidence document', async t => {
  const { directory } = await fixture(t);
  await rm(resolve(directory, 'BUILD_EVIDENCE.md'));
  await assert.rejects(verifyPdfiumNotices(directory), /ENOENT/);
});

test('PDFium: distributed notices and deployment assets match the verified source', async () => {
  const manifest = await verifyPdfiumNotices(source);
  await verifyPdfiumNotices(resolve(distribution, 'assets/pdf/pdfium'));
  const paths = ['SOURCE.md', 'NOTICE.md', 'BUILD_EVIDENCE.md', 'provenance.json', ...manifest.licenseFiles.map(entry => entry.path)];
  for (const target of ['licenses/pdfium', 'assets/pdf/pdfium']) {
    for (const path of paths) {
      assert.deepEqual(await readFile(resolve(distribution, target, path)), await readFile(resolve(source, path)), `${target}/${path}`);
    }
  }
});
