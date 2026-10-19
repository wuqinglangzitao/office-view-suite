import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { writeAssetManifest, verifyAssetManifest } from '../scripts/sdk-asset-manifest.mjs';

async function fixture(t) {
  const dir = await mkdtemp(resolve(tmpdir(), 'office-asset-manifest-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await mkdir(resolve(dir, 'assets/excel'), { recursive: true });
  await writeFile(resolve(dir, 'assets/excel/worker.js'), 'worker-v1');
  await writeFile(resolve(dir, 'assets/excel/excel_view_rs_bg.wasm'), Buffer.from([0, 97, 115, 109]));
  return dir;
}

test('asset index records deployable paths and verifiable hashes', async t => {
  const dir = await fixture(t);
  const manifest = await writeAssetManifest(dir, 'excel');
  assert.equal(manifest.version, 1);
  assert.equal(manifest.format, 'excel');
  assert.deepEqual(manifest.assets.map(asset => asset.path), [
    'assets/excel/excel_view_rs_bg.wasm', 'assets/excel/worker.js',
  ]);
  const worker = manifest.assets.find(asset => asset.path.endsWith('worker.js'));
  assert.equal(worker.bytes, 9);
  assert.equal(worker.sha256, createHash('sha256').update('worker-v1').digest('hex'));
  assert.equal(worker.contentType, 'text/javascript');
  assert.deepEqual(await verifyAssetManifest(dir, 'excel'), manifest);
});

test('packaging rejects a modified file even if its size is unchanged', async t => {
  const dir = await fixture(t);
  await writeAssetManifest(dir, 'excel');
  await writeFile(resolve(dir, 'assets/excel/worker.js'), 'worker-v2');
  await assert.rejects(verifyAssetManifest(dir, 'excel'), /hash|integrity/i);
});

test('a required WASM cannot disappear from both the directory and manifest', async t => {
  const dir = await fixture(t);
  const manifest = await writeAssetManifest(dir, 'excel');
  await rm(resolve(dir, 'assets/excel/excel_view_rs_bg.wasm'));
  manifest.assets = manifest.assets.filter(asset => asset.path.endsWith('worker.js'));
  await writeFile(resolve(dir, 'asset-manifest.json'), JSON.stringify(manifest));
  await assert.rejects(verifyAssetManifest(dir, 'excel'), /missing|required/i);
  await assert.rejects(writeAssetManifest(dir, 'excel'), /missing|required/i);
});

test('unlisted deployment files and invalid manifest identities are rejected', async t => {
  const dir = await fixture(t);
  await writeAssetManifest(dir, 'excel');
  await writeFile(resolve(dir, 'assets/excel/extra.js'), 'unlisted');
  await assert.rejects(verifyAssetManifest(dir, 'excel'), /inventory|listing/i);
  await rm(resolve(dir, 'assets/excel/extra.js'));
  const manifest = JSON.parse(await readFile(resolve(dir, 'asset-manifest.json'), 'utf8'));
  for (const invalid of [
    { ...manifest, version: 99 },
    { ...manifest, format: 'word' },
    { ...manifest, assets: [...manifest.assets, manifest.assets[0]] },
    { ...manifest, assets: [{ ...manifest.assets[0], path: '../outside.wasm' }, manifest.assets[1]] },
  ]) {
    await writeFile(resolve(dir, 'asset-manifest.json'), JSON.stringify(invalid));
    await assert.rejects(verifyAssetManifest(dir, 'excel'), /manifest|inventory/i);
  }
});
