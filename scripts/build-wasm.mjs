import { execFileSync } from 'node:child_process';
import { rm } from 'node:fs/promises';
import { captureWasmInputs, recordWasmBuild, wasmLayout } from './wasm-build-state.mjs';

const format = process.argv[2];
const layout = wasmLayout(format);
const version = command => execFileSync(command, ['--version'], { cwd: layout.directory, encoding: 'utf8' }).trim();
const tools = { rustc: version('rustc'), cargo: version('cargo'), wasmPack: version('wasm-pack') };
const inputs = await captureWasmInputs(format);
// A failed rebuild must never leave a previous successful build certified.
await rm(layout.record, { force: true });
execFileSync('wasm-pack', ['build', 'rs', '--target', layout.target, '--out-dir', '../src/wasm', '--release', '--locked'], {
  cwd: layout.directory, stdio: 'inherit',
});
await recordWasmBuild(format, inputs, tools);
console.log(`Recorded ${format} WASM build inputs and output hashes.`);
