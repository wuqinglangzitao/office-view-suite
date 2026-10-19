import { createHash } from 'node:crypto';
import { readFile, readdir, rename, writeFile } from 'node:fs/promises';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('../', import.meta.url));
const ignoredDirectories = new Set(['target', '.git', 'node_modules']);
const hash = data => createHash('sha256').update(data).digest('hex');

export function wasmLayout(format, root = repository) {
  if (!['word', 'excel', 'ppt', 'pdf'].includes(format)) throw new Error(`Unknown WASM format: ${format}`);
  const directory = resolve(root, `packages/viewer-${format}`);
  const output = resolve(directory, 'src/wasm');
  const stem = format === 'pdf' ? 'pdfium_view_rs' : `${format}_view_rs`;
  return {
    directory, crate: resolve(directory, 'rs'), output,
    target: format === 'pdf' ? 'no-modules' : 'web',
    outputs: ['.js', '.d.ts', '_bg.wasm'].map(suffix => `${stem}${suffix}`),
    record: resolve(output, 'build-state.json'),
  };
}

async function sourceFiles(directory, optional = false) {
  let entries;
  try { entries = await readdir(directory, { withFileTypes: true }); }
  catch (error) { if (optional && error.code === 'ENOENT') return []; throw error; }
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      if (!ignoredDirectories.has(entry.name)) files.push(...await sourceFiles(path));
    } else if (entry.isFile()) files.push(path);
    else throw new Error(`WASM build inputs must be regular files: ${path}`);
  }
  return files;
}

/** Fingerprint local build inputs, including vendored source and Cargo configuration. */
export async function captureWasmInputs(format, root = repository) {
  const layout = wasmLayout(format, root);
  const files = [
    ...await sourceFiles(layout.crate),
    resolve(root, 'rust-toolchain.toml'),
    resolve(root, 'scripts/build-wasm.mjs'),
    resolve(root, 'scripts/wasm-build-state.mjs'),
  ];
  // Cargo also reads configuration from every ancestor of the crate.
  // The crate's own .cargo directory is already covered by its source tree.
  for (let directory = layout.directory; ; directory = dirname(directory)) {
    files.push(...await sourceFiles(resolve(directory, '.cargo'), true));
    if (directory === resolve(root)) break;
  }
  const entries = [];
  for (const path of files.sort()) entries.push([relative(root, path).replaceAll('\\', '/'), hash(await readFile(path))]);
  return hash(JSON.stringify({ format, target: layout.target, profile: 'release', locked: true, files: entries }));
}

async function describeOutputs(layout) {
  return Promise.all(layout.outputs.map(async path => {
    const contents = await readFile(resolve(layout.output, path));
    if (!contents.length) throw new Error(`Empty WASM build output: ${path}`);
    return { path, bytes: contents.byteLength, sha256: hash(contents) };
  }));
}

/** Called only after wasm-pack succeeds, using the input fingerprint taken before compilation. */
export async function recordWasmBuild(format, inputs, tools, root = repository) {
  if (await captureWasmInputs(format, root) !== inputs) throw new Error('WASM sources changed during compilation; rebuild before packaging.');
  const layout = wasmLayout(format, root);
  const state = { version: 1, format, inputs, tools, outputs: await describeOutputs(layout) };
  await writeFile(`${layout.record}.tmp`, JSON.stringify(state, null, 2) + '\n');
  await rename(`${layout.record}.tmp`, layout.record);
  return state;
}

export async function verifyWasmBuild(format, root = repository) {
  const layout = wasmLayout(format, root);
  try {
    const state = JSON.parse(await readFile(layout.record, 'utf8'));
    if (state.version !== 1 || state.format !== format || state.inputs !== await captureWasmInputs(format, root)) {
      throw new Error('build inputs have changed');
    }
    if (JSON.stringify(state.outputs) !== JSON.stringify(await describeOutputs(layout))) throw new Error('generated output has changed');
    return state;
  } catch (cause) {
    throw new Error(`Missing or stale ${format} WASM (${cause.message}). Run pnpm wasm:${format} before building the SDK.`, { cause });
  }
}
