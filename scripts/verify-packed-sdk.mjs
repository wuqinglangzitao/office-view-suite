import { mkdtemp, mkdir, cp, writeFile, readdir, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { verifyPdfiumNotices } from './verify-pdfium-notices.mjs';
import { verifyAssetManifest } from './sdk-asset-manifest.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const projectLicense = await readFile(resolve(root, 'LICENSE'));
const pdfiumSource = resolve(root, 'third_party/pdfium');
const pdfium = await verifyPdfiumNotices(pdfiumSource);
const consumer = await mkdtemp(resolve(tmpdir(), 'office-sdk-consumer-'));
const packages = ['viewer-core', 'viewer-word', 'viewer-excel', 'viewer-ppt', 'viewer-pdf', 'viewer'];
const tarballs = resolve(consumer, 'tarballs');
await mkdir(tarballs);
const run = (bin, args, cwd = consumer) => execFileSync(bin, args, { cwd, stdio: 'inherit' });
for (const name of packages) run('pnpm', ['pack', '--pack-destination', tarballs], resolve(root, 'packages', name));
const dependencies = {};
for (const name of packages) {
  const file = `office-view-suite-${name}-0.1.0.tgz`;
  const listing = execFileSync('tar', ['-tzf', resolve(tarballs, file)], { encoding: 'utf8' });
  if (/\/(?:node_modules|vendor|target|samples|src)\//m.test(listing)) throw new Error(`Unexpected package files: ${name}`);
  const packedLicense = execFileSync('tar', ['-xOf', resolve(tarballs, file), 'package/dist/LICENSE']);
  if (!packedLicense.equals(projectLicense)) throw new Error(`Packed project license differs from source: ${name}`);
  const packedMetadata = JSON.parse(execFileSync('tar', ['-xOf', resolve(tarballs, file), 'package/package.json'], { encoding: 'utf8' }));
  if (packedMetadata.license !== 'Apache-2.0') throw new Error(`Missing Apache-2.0 package metadata: ${name}`);
  if (name === 'viewer-pdf') {
    const entries = new Set(listing.trim().split('\n'));
    const documents = ['SOURCE.md', 'NOTICE.md', 'BUILD_EVIDENCE.md', 'provenance.json', ...pdfium.licenseFiles.map(entry => entry.path)];
    for (const prefix of ['package/dist/assets/pdf/pdfium', 'package/dist/licenses/pdfium']) {
      const paths = prefix.includes('/assets/') ? [...documents, ...pdfium.artifact.files.map(entry => entry.path)] : documents;
      for (const path of paths) {
        const member = `${prefix}/${path}`;
        if (!entries.has(member)) throw new Error(`Missing packed PDFium material: ${member}`);
        const contents = execFileSync('tar', ['-xOf', resolve(tarballs, file), member], { maxBuffer: 32 * 1024 * 1024 });
        const expected = await readFile(resolve(pdfiumSource, path));
        const hash = data => createHash('sha256').update(data).digest('hex');
        if (hash(contents) !== hash(expected)) throw new Error(`Packed PDFium material differs from verified source: ${member}`);
      }
    }
  }
  dependencies[`@office-view-suite/${name}`] = `file:./tarballs/${file}`;
}
// Only packed runtime dependencies are installed. Build tools are reused from this checkout.
await writeFile(resolve(consumer, 'package.json'), JSON.stringify({ private: true, type: 'module', dependencies, pnpm: { overrides: dependencies } }, null, 2));
await cp(resolve(root, 'fixtures/vanilla-vite/src'), resolve(consumer, 'src'), { recursive: true });
await cp(resolve(root, 'fixtures/vanilla-vite/index.html'), resolve(consumer, 'index.html'));
const tsconfig = JSON.parse(await readFile(resolve(root, 'fixtures/vanilla-vite/tsconfig.json')));
delete tsconfig.compilerOptions.types;
await writeFile(resolve(consumer, 'tsconfig.json'), JSON.stringify(tsconfig));
await writeFile(resolve(consumer, 'src/env.d.ts'), 'interface ImportMeta { env: { BASE_URL: string }; hot?: { dispose(callback: () => void): void } }\n');
await writeFile(resolve(consumer, 'vite.config.js'), "export default { base: '/office-view/', build: { target: 'es2022' } };\n");
await mkdir(resolve(consumer, 'public/samples'), { recursive: true });
for (const [format, sample] of Object.entries({word:'styles-word.docx',excel:'styles-excel.xlsx',ppt:'styles-ppt.pptx',pdf:'handmade-mixed.pdf'})) {
  await cp(resolve(root, `apps/${format}-view/public/samples/${sample}`), resolve(consumer, 'public/samples', sample));
}
run('pnpm', ['install', '--offline', '--ignore-scripts']);
for (const format of ['word', 'excel', 'ppt', 'pdf']) {
  const dist = resolve(consumer, `node_modules/@office-view-suite/viewer-${format}/dist`);
  const manifest = await verifyAssetManifest(dist, format);
  for (const asset of manifest.assets) {
    const deployed = resolve(consumer, 'public/office-assets', asset.path.slice('assets/'.length));
    await mkdir(resolve(deployed, '..'), { recursive: true });
    await cp(resolve(dist, asset.path), deployed);
  }
}
run('node', [resolve(root, 'packages/viewer/node_modules/typescript/bin/tsc'), '--noEmit']);
run('node', [resolve(root, 'packages/viewer/node_modules/vite/bin/vite.js'), 'build']);
// Native ESM consumer uses an import map and the exact same extracted package output.
const imports = {};
for (const name of packages) {
  await cp(resolve(consumer, `node_modules/@office-view-suite/${name}/dist`), resolve(consumer, `dist/esm/${name}`), { recursive: true });
  imports[`@office-view-suite/${name}`] = `./esm/${name}/index.js`;
}
await writeFile(resolve(consumer, 'dist/native.html'), `<!doctype html><meta charset="utf-8"><title>Native ESM SDK</title><div id="viewer"></div><script type="importmap">${JSON.stringify({imports})}</script><script type="module">import {createOfficeViewer} from '@office-view-suite/viewer'; window.nativeViewer=createOfficeViewer(document.querySelector('#viewer'));</script>`);
await mkdir(resolve(root, 'output'), { recursive: true });
await writeFile(resolve(root, 'output/packed-consumer.json'), JSON.stringify({ consumer, files: await readdir(tarballs) }, null, 2));
console.log(`Packed SDK consumer verified: ${consumer}`);
