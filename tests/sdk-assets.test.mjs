import test from 'node:test';
import assert from 'node:assert/strict';
import { formatBytes } from './sdk-format-fixtures.mjs';

const formats = { word: 'docx', excel: 'xlsx', ppt: 'pptx', pdf: 'pdf' };
for (const [kind, format] of Object.entries(formats)) {
  test(`${kind}: baseUrl is a directory with or without a trailing slash`, async () => {
    globalThis.document = { baseURI: 'https://host.example/app/' };
    const module = await import(`../packages/viewer-${kind}/dist/index.js`);
    const create = module[`create${kind[0].toUpperCase()}${kind.slice(1)}Adapter`];
    for (const baseUrl of ['/static/office', '/static/office/', new URL('https://cdn.example/office'), 'office-assets']) {
      const abort = new AbortController();
      let actual;
      const adapter = create({ assets: { baseUrl }, workerFactory(url) {
        actual = url.href;
        return { terminate() {}, postMessage() { queueMicrotask(() => abort.abort()); } };
      } });
      await assert.rejects(adapter.open({ bytes: formatBytes(format), format, signal: abort.signal }), error => error.code === 'ABORTED');
      const base = new URL(String(baseUrl), document.baseURI);
      if (!base.pathname.endsWith('/')) base.pathname += '/';
      assert.equal(actual, new URL(`${kind}/worker.js`, base).href);
      adapter.destroy();
    }
    delete globalThis.document;
  });
}
