import { createWorkerAdapter, type OfficeOpenRequest, type OfficeOpenResult, type OfficeViewerOptions } from '@office-view-suite/viewer-core';
import { renderWordInto } from './core/render';
import type { WordDoc } from './core/types';

export function createWordAdapter(options: OfficeViewerOptions = {}) {
  const bundledWorker = new URL('./assets/word/worker.js?no-inline', import.meta.url);
  const bundledWasm = new URL('./assets/word/word_view_rs_bg.wasm?no-inline', import.meta.url);
  const base = options.assets?.baseUrl ? new URL(String(options.assets.baseUrl), document.baseURI) : undefined;
  if (base && !base.pathname.endsWith('/')) base.pathname += '/';
  const assetUrl = (override: string | undefined, path: string, bundled: URL): URL =>
    override ? new URL(override, document.baseURI) :
    base ? new URL(path, base) : bundled;
  const workerUrl = assetUrl(options.assets?.word?.workerUrl, 'word/worker.js', bundledWorker);
  const wasmUrl = assetUrl(options.assets?.word?.wasmUrl, 'word/word_view_rs_bg.wasm', bundledWasm).href;
  const workerAdapter = createWorkerAdapter<WordDoc>({
    format: 'docx',
    formats: ['doc', 'docx', 'docm'],
    workerData: { wasmUrl },
    createWorker: () => options.workerFactory
      ? options.workerFactory(workerUrl, { type: 'module' })
      : new Worker(workerUrl, { type: 'module' }),
  });

  return {
    async open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: WordDoc }> {
      const result = await workerAdapter.open(request);
      return {
        format: request.format ?? 'docx',
        durationMs: result.durationMs,
        stats: { paragraphs: result.document.stats.paragraphs, tables: result.document.stats.tables, images: result.document.stats.images },

        warnings: result.warnings,
        document: result.document,
      };
    },
    render(host: HTMLElement, document: WordDoc) {
      const rendered = renderWordInto(host, document);
      return { ...rendered, stats: { ...rendered.stats } };
    },
    destroy: workerAdapter.destroy,
  };
}
