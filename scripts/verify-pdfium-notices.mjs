import { createHash } from 'node:crypto';
import { readFile, realpath, stat } from 'node:fs/promises';
import { isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const sourceDirectory = fileURLToPath(new URL('../apps/pdf-view/public/pdfium/', import.meta.url));
const binaryNames = ['pdfium.std.js', 'pdfium.std.wasm'];
const sha256Pattern = /^[a-f0-9]{64}$/;

function requireString(value, label) {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`Missing or invalid ${label}`);
}

function validateRelativePath(path) {
  requireString(path, 'relative file path');
  if (isAbsolute(path) || /[\\:\0]/.test(path) || path.split('/').some(part => !part || part === '.' || part === '..')) {
    throw new Error(`Unsafe relative file path: ${path}`);
  }
}

async function readContainedFile(directory, path) {
  validateRelativePath(path);
  const actual = await realpath(resolve(directory, path));
  const local = relative(directory, actual);
  if (!local || isAbsolute(local) || local === '..' || local.startsWith(`..${sep}`)) {
    throw new Error(`File escapes PDFium directory: ${path}`);
  }
  if (!(await stat(actual)).isFile()) throw new Error(`Not a regular file: ${path}`);
  const contents = await readFile(actual);
  if (!contents.length) throw new Error(`Empty PDFium file: ${path}`);
  return contents;
}

async function verifyHash(directory, entry) {
  if (!sha256Pattern.test(entry.sha256)) throw new Error(`Invalid SHA-256 for ${entry.path}`);
  const contents = await readContainedFile(directory, entry.path);
  const digest = createHash('sha256').update(contents).digest('hex');
  if (digest !== entry.sha256) throw new Error(`SHA-256 mismatch: ${entry.path}`);
}

/** Verify the checked-in evidence without downloading anything or asserting legal completeness. */
export async function verifyPdfiumNotices(directory = sourceDirectory) {
  const root = await realpath(directory);
  const manifest = JSON.parse(await readContainedFile(root, 'provenance.json'));
  if (manifest.schemaVersion !== 1) throw new Error('Unsupported PDFium provenance schemaVersion');
  requireString(manifest.artifact?.release, 'artifact.release');
  requireString(manifest.artifact.archive?.url, 'artifact.archive.url');
  if (!sha256Pattern.test(manifest.artifact.archive?.sha256)) throw new Error('Invalid artifact.archive.sha256');
  if (!Array.isArray(manifest.artifact.files) || manifest.artifact.files.length !== binaryNames.length) {
    throw new Error('PDFium provenance must identify the JavaScript and WASM binary pair');
  }
  const files = new Set();
  for (const entry of manifest.artifact.files) {
    if (!binaryNames.includes(entry?.path) || files.has(entry.path)) throw new Error('Invalid or duplicate PDFium binary path');
    files.add(entry.path);
    validateRelativePath(entry.archivePath);
    await verifyHash(root, entry);
  }

  if (!Array.isArray(manifest.licenseFiles) || !manifest.licenseFiles.length) throw new Error('PDFium licenseFiles are required');
  const licenses = new Set();
  for (const entry of manifest.licenseFiles) {
    validateRelativePath(entry?.path);
    if (!entry.path.startsWith('licenses/') || licenses.has(entry.path)) throw new Error(`Invalid or duplicate license path: ${entry.path}`);
    licenses.add(entry.path);
    requireString(entry.sourceUrl, `sourceUrl for ${entry.path}`);
    await verifyHash(root, entry);
  }
  if (!Array.isArray(manifest.components) || !manifest.components.length) throw new Error('PDFium components are required');
  for (const component of manifest.components) {
    for (const field of ['name', 'version', 'source', 'license']) requireString(component?.[field], `component.${field}`);
    if (!Array.isArray(component.licenseFiles) || !component.licenseFiles.length) {
      throw new Error(`Missing license references: ${component.name}`);
    }
    for (const path of component.licenseFiles) {
      if (!licenses.has(path)) throw new Error(`Unknown license reference for ${component.name}: ${path}`);
    }
  }
  for (const path of ['SOURCE.md', 'NOTICE.md', 'BUILD_EVIDENCE.md']) {
    const contents = await readContainedFile(root, path);
    if (!contents.toString('utf8').trim()) throw new Error(`Empty PDFium document: ${path}`);
  }
  return manifest;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const manifest = await verifyPdfiumNotices(process.argv[2]);
  console.log(`PDFium ${manifest.artifact.release}: binary hashes, ${manifest.licenseFiles.length} license files, and ${manifest.components.length} component references verified (offline).`);
}
