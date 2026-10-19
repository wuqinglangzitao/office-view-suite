import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { captureWasmInputs, recordWasmBuild, verifyWasmBuild, wasmLayout } from '../scripts/wasm-build-state.mjs';

async function fixture(t) {
  const root = await mkdtemp(resolve(tmpdir(), 'office-wasm-build-test-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const files = {
    'rust-toolchain.toml': '[toolchain]\nchannel = "1.89.0"\n',
    'scripts/build-wasm.mjs': '// build command\n',
    'scripts/wasm-build-state.mjs': '// fingerprint contract\n',
    'packages/viewer-excel/rs/Cargo.toml': '[package]\nname = "excel-view-rs"\n',
    'packages/viewer-excel/rs/Cargo.lock': 'version = 4\n',
    'packages/viewer-excel/rs/src/lib.rs': 'pub fn parse() {}\n',
    'packages/viewer-excel/rs/vendor/parser/src/lib.rs': 'pub fn read() {}\n',
    'packages/viewer-excel/src/wasm/excel_view_rs.js': 'export class Engine {}\n',
    'packages/viewer-excel/src/wasm/excel_view_rs.d.ts': 'export class Engine {}\n',
    'packages/viewer-excel/src/wasm/excel_view_rs_bg.wasm': Buffer.from([0, 97, 115, 109]),
  };
  const write = async (path, contents) => {
    await mkdir(dirname(resolve(root, path)), { recursive: true });
    await writeFile(resolve(root, path), contents);
  };
  for (const [path, contents] of Object.entries(files)) await write(path, contents);
  const inputs = await captureWasmInputs('excel', root);
  await recordWasmBuild('excel', inputs, { rustc: 'test compiler', wasmPack: 'test builder' }, root);
  return { root, write, inputs };
}

test('fresh WASM build verifies without any demo directories', async t => {
  const { root } = await fixture(t);
  const state = await verifyWasmBuild('excel', root);
  assert.equal(state.format, 'excel');
  assert.equal(state.tools.rustc, 'test compiler');
  assert.equal(state.outputs.length, 3);
  const layout = wasmLayout('pdf', root);
  assert.equal(layout.crate, resolve(root, 'packages/viewer-pdf/rs'));
  assert.equal(layout.target, 'no-modules');
});

for (const path of [
  'packages/viewer-excel/rs/src/lib.rs',
  'packages/viewer-excel/rs/vendor/parser/src/lib.rs',
  'packages/viewer-excel/rs/Cargo.lock',
  'rust-toolchain.toml',
  'scripts/build-wasm.mjs',
  'scripts/wasm-build-state.mjs',
]) {
  test(`WASM rejects stale build after changing ${path}`, async t => {
    const { root, write } = await fixture(t);
    await write(path, 'changed');
    await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
  });
}

test('WASM detects added and deleted source files', async t => {
  const { root, write } = await fixture(t);
  const path = 'packages/viewer-excel/rs/src/added.rs';
  await write(path, 'pub fn extra() {}');
  await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
  await rm(resolve(root, path));
  await verifyWasmBuild('excel', root);
  await rm(resolve(root, 'packages/viewer-excel/rs/src/lib.rs'));
  await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
});

test('WASM ignores compiler caches', async t => {
  const { root, write } = await fixture(t);
  await write('packages/viewer-excel/rs/target/debug/cache', 'cache');
  await write('packages/viewer-excel/rs/vendor/parser/target/debug/cache', 'cache');
  await verifyWasmBuild('excel', root);
});

for (const directory of ['', 'packages/', 'packages/viewer-excel/', 'packages/viewer-excel/rs/']) {
  test(`WASM includes Cargo configuration at ${directory || 'repository root'}`, async t => {
    const { root, write } = await fixture(t);
    await write(`${directory}.cargo/config.toml`, '[build]\nrustflags = ["--cfg", "changed"]');
    await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
  });
}

for (const suffix of ['.js', '.d.ts', '_bg.wasm']) {
  test(`WASM rejects modified or missing output ${suffix}`, async t => {
    const { root, write } = await fixture(t);
    const path = `packages/viewer-excel/src/wasm/excel_view_rs${suffix}`;
    await write(path, 'tampered output');
    await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
    await rm(resolve(root, path));
    await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
  });
}

test('WASM requires a valid record from a successful build', async t => {
  const { root, write } = await fixture(t);
  const path = 'packages/viewer-excel/src/wasm/build-state.json';
  await rm(resolve(root, path));
  await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
  await write(path, '{invalid');
  await assert.rejects(verifyWasmBuild('excel', root), /pnpm wasm:excel/);
});

test('WASM does not certify a build when sources changed during compilation', async t => {
  const { root, write, inputs } = await fixture(t);
  const before = await readFile(resolve(root, 'packages/viewer-excel/src/wasm/build-state.json'));
  await write('packages/viewer-excel/rs/src/lib.rs', 'pub fn changed() {}');
  await assert.rejects(recordWasmBuild('excel', inputs, {}, root), /changed during/);
  assert.deepEqual(await readFile(resolve(root, 'packages/viewer-excel/src/wasm/build-state.json')), before);
});
