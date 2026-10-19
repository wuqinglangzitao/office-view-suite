import { cp, mkdir, access } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { verifyWasmBuild, wasmLayout } from './wasm-build-state.mjs';

// Run from an adapter package. Generated assets are never published from src/.
const format = process.argv[2];
if (!['word', 'excel', 'ppt', 'pdf'].includes(format)) throw new Error('Unknown adapter');
const root = process.cwd();
const layout = wasmLayout(format);
await verifyWasmBuild(format);
const out = resolve(root, `src/assets/${format}`);
await mkdir(out, { recursive: true });
if (format === 'pdf') {
  await access(resolve(layout.output, 'pdfium_view_rs_bg.wasm'));
  for (const dir of ['pdfium', 'glue']) {
    await mkdir(resolve(out, dir), { recursive: true });
    const prefix = dir === 'pdfium' ? 'pdfium.std' : 'pdfium_view_rs';
    const source = dir === 'pdfium' ? resolve(root, '../../third_party/pdfium') : layout.output;
    for (const file of [`${prefix}.js`, dir === 'pdfium' ? `${prefix}.wasm` : `${prefix}_bg.wasm`]) {
      await cp(resolve(source, file), resolve(out, dir, file));
    }
  }
  await cp(resolve(root, 'src/worker/pdf-worker.js'), resolve(out, 'worker.js'));
} else {
  const { build } = await import(pathToFileURL(resolve(root, 'node_modules/vite/dist/node/index.js')).href);
  await build({
    configFile: false,
    root,
    base: './',
    publicDir: false,
    build: {
      target: 'es2022',
      outDir: out,
      emptyOutDir: true,
      assetsInlineLimit: 0,
      rollupOptions: {
        input: resolve(root, 'src/worker/worker.ts'),
        output: { format: 'es', entryFileNames: 'worker.js', assetFileNames: '[name][extname]', inlineDynamicImports: true },
      },
    },
  });
}
