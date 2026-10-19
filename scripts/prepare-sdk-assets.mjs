import { cp, mkdir, access } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Run from an adapter package. Generated assets are never published from src/.
const format = process.argv[2];
if (!['word', 'excel', 'ppt', 'pdf'].includes(format)) throw new Error('Unknown adapter');
const root = process.cwd();
const app = resolve(root, `../../apps/${format}-view`);
const out = resolve(root, `src/assets/${format}`);
await mkdir(out, { recursive: true });
if (format === 'pdf') {
  await access(resolve(app, 'public/glue/pdfium_view_rs_bg.wasm'));
  for (const dir of ['pdfium', 'glue']) {
    await mkdir(resolve(out, dir), { recursive: true });
    const prefix = dir === 'pdfium' ? 'pdfium.std' : 'pdfium_view_rs';
    for (const file of [`${prefix}.js`, dir === 'pdfium' ? `${prefix}.wasm` : `${prefix}_bg.wasm`]) {
      await cp(resolve(app, 'public', dir, file), resolve(out, dir, file));
    }
  }
  await cp(resolve(root, 'src/worker/pdf-worker.js'), resolve(out, 'worker.js'));
} else {
  // Rust crates remain in apps; a rebuild of WASM is picked up on every SDK build.
  const wasmDir = resolve(root, 'src/wasm');
  await mkdir(wasmDir, { recursive: true });
  for (const suffix of ['.js', '.d.ts', '_bg.wasm']) {
    await cp(resolve(app, `src/wasm/${format}_view_rs${suffix}`), resolve(wasmDir, `${format}_view_rs${suffix}`));
  }
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
