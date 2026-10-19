import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';

for (const name of ['viewer-core', 'viewer-word', 'viewer-excel', 'viewer-ppt', 'viewer-pdf', 'viewer']) {
  test(`${name}: exported JavaScript and declarations exist`, async () => {
    const dir = new URL(`../packages/${name}/`, import.meta.url);
    const pkg = JSON.parse(await readFile(new URL('package.json', dir)));
    for (const entry of Object.values(pkg.exports)) {
      for (const target of Object.values(entry)) {
        assert.ok(target.startsWith('./'), `invalid export: ${target}`);
        await access(new URL(target, dir));
      }
    }
    const js = await readFile(new URL('dist/index.js', dir), 'utf8');
    assert.ok(!js.includes('data:video/mp2t'), 'worker contains uncompiled TypeScript');
    assert.ok(!js.includes('data:application/wasm'), 'WASM must be a separate asset');
    if (/viewer-(word|excel|ppt|pdf)$/.test(name)) {
      await access(new URL('dist/THIRD_PARTY_NOTICES.md', dir));
      const dependency = { 'viewer-word': 'rwml-0.1.4', 'viewer-excel': 'umya-spreadsheet-3.0.0', 'viewer-ppt': 'ppt-rs-0.2.7' }[name];
      if (dependency) {
        const { readdir } = await import('node:fs/promises');
        const entries = await readdir(new URL('dist/licenses/', dir));
        assert.ok(entries.some(entry => entry.startsWith(dependency.replace(/-\d.*$/, '') + '-')), 'upstream notices missing');
      }
    }
  });
}
