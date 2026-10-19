import { execFileSync } from 'node:child_process';
import { readdir, mkdir, cp, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { verifyPdfiumNotices } from './verify-pdfium-notices.mjs';
import { writeAssetManifest } from './sdk-asset-manifest.mjs';
import { verifyWasmBuild } from './wasm-build-state.mjs';

// Include the actual license/notice files of crates statically linked into WASM.
const format = process.argv[2];
await verifyWasmBuild(format);
const crate = resolve('rs');
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'wasm32-unknown-unknown'], {
  cwd: crate, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
}));
const notices = resolve('dist/licenses');
await mkdir(notices, { recursive: true });
const lines = ['# Bundled Rust dependencies', '', 'License files below are copied from the local sources whose build fingerprint is verified before SDK packaging. Parser crates under packages/viewer-*/rs/vendor contain project modifications; their original copyright notices are preserved.', ''];
for (const pkg of metadata.packages) {
  const dir = dirname(pkg.manifest_path);
  const candidates = (await readdir(dir)).filter(name => /^(licen[cs]e|notice|copying)([.-]|$)/i.test(name));
  if (pkg.license_file && !candidates.includes(pkg.license_file)) candidates.push(pkg.license_file);
  if (candidates.length) {
    const target = resolve(notices, `${pkg.name}-${pkg.version}`);
    await mkdir(target, { recursive: true });
    for (const file of candidates) await cp(resolve(dir, file), resolve(target, file), { recursive: true });
  }
  lines.push(`- ${pkg.name} ${pkg.version}: ${pkg.license ?? 'see source license'}; ${pkg.repository ?? pkg.source ?? 'local source'}`);
}
if (format === 'pdf') {
  const source = resolve('../../third_party/pdfium');
  await verifyPdfiumNotices(source);
  const runtime = resolve('dist/assets/pdf/pdfium');
  for (const target of [resolve(notices, 'pdfium'), runtime]) {
    await mkdir(target, { recursive: true });
    for (const file of ['SOURCE.md', 'NOTICE.md', 'BUILD_EVIDENCE.md', 'provenance.json', 'licenses']) {
      await cp(resolve(source, file), resolve(target, file), { recursive: true });
    }
  }
  // Check the actual runtime files too: notices must never accompany a stale binary pair.
  await verifyPdfiumNotices(runtime);
  lines.push('', '## PDFium prebuilt runtime', '', 'The binary provenance, component inventory, and preserved license texts are in [licenses/pdfium/NOTICE.md](licenses/pdfium/NOTICE.md). See [BUILD_EVIDENCE.md](licenses/pdfium/BUILD_EVIDENCE.md) for the scope and limitations of the upstream build evidence. The same materials accompany the deployed runtime under assets/pdf/pdfium/. Binary and license-file hashes are checked during packaging.');
}
await writeFile(resolve('dist/THIRD_PARTY_NOTICES.md'), lines.join('\n') + '\n');
await writeAssetManifest(resolve('dist'), format);
