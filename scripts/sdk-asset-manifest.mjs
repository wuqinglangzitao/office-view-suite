import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { extname, resolve } from 'node:path';

const runtimes = {
  word: ['worker.js', 'word_view_rs_bg.wasm'],
  excel: ['worker.js', 'excel_view_rs_bg.wasm'],
  ppt: ['worker.js', 'ppt_view_rs_bg.wasm'],
  pdf: ['worker.js', 'glue/pdfium_view_rs.js', 'glue/pdfium_view_rs_bg.wasm', 'pdfium/pdfium.std.js', 'pdfium/pdfium.std.wasm'],
};
const contentTypes = {
  '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json',
  '.md': 'text/markdown', '.txt': 'text/plain',
};

async function inventory(dist, format) {
  if (!Object.hasOwn(runtimes, format)) throw new Error(`Unknown asset manifest format: ${format}`);
  const paths = [];
  async function visit(relative) {
    for (const entry of await readdir(resolve(dist, relative), { withFileTypes: true })) {
      const path = `${relative}/${entry.name}`;
      if (entry.isDirectory()) await visit(path);
      else if (entry.isFile()) paths.push(path);
      else throw new Error(`Unsupported deployment asset: ${path}`);
    }
  }
  await visit(`assets/${format}`);
  for (const name of runtimes[format]) {
    if (!paths.includes(`assets/${format}/${name}`)) throw new Error(`Missing required runtime asset: ${format}/${name}`);
  }
  return paths.sort();
}

async function describe(dist, path) {
  const data = await readFile(resolve(dist, path));
  return {
    path,
    bytes: data.byteLength,
    sha256: createHash('sha256').update(data).digest('hex'),
    contentType: contentTypes[extname(path)] ?? 'application/octet-stream',
  };
}

/** Build-time deployment index; browser resource URLs remain statically bundled. */
export async function writeAssetManifest(dist, format) {
  const paths = await inventory(dist, format);
  const manifest = { version: 1, format, assets: await Promise.all(paths.map(path => describe(dist, path))) };
  await writeFile(resolve(dist, 'asset-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  return manifest;
}

/** Verify the extracted package, including files missing from both disk and the index. */
export async function verifyAssetManifest(dist, format) {
  const manifest = JSON.parse(await readFile(resolve(dist, 'asset-manifest.json'), 'utf8'));
  if (manifest.version !== 1 || manifest.format !== format || !Array.isArray(manifest.assets)) {
    throw new Error(`Invalid asset manifest: ${format}`);
  }
  const paths = await inventory(dist, format);
  // Compare names before resolving any paths supplied by the manifest.
  if (JSON.stringify(manifest.assets.map(asset => asset?.path)) !== JSON.stringify(paths)) {
    throw new Error(`Asset manifest inventory mismatch: ${format}`);
  }
  for (const asset of manifest.assets) {
    const actual = await describe(dist, asset.path);
    if (asset.bytes !== actual.bytes || asset.sha256 !== actual.sha256 || asset.contentType !== actual.contentType) {
      throw new Error(`Asset integrity/hash mismatch: ${asset.path}`);
    }
  }
  return manifest;
}
