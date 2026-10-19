import { createWorkerAdapter, type OfficeOpenRequest, type OfficeOpenResult, type OfficeViewerOptions } from '@office-view-suite/viewer-core';
import { renderPptInto } from './core/render';
import type { PptDoc } from './core/types';

export function createPptAdapter(options: OfficeViewerOptions = {}) {
  const bundledWorker = new URL('./assets/ppt/worker.js?no-inline', import.meta.url);
  const bundledWasm = new URL('./assets/ppt/ppt_view_rs_bg.wasm?no-inline', import.meta.url);
  const base = options.assets?.baseUrl ? new URL(String(options.assets.baseUrl), document.baseURI) : undefined;
  if (base && !base.pathname.endsWith('/')) base.pathname += '/';
  const assetUrl = (override: string | undefined, path: string, bundled: URL): URL =>
    override ? new URL(override, document.baseURI) :
    base ? new URL(path, base) : bundled;
  const workerUrl = assetUrl(options.assets?.ppt?.workerUrl, 'ppt/worker.js', bundledWorker);
  const wasmUrl = assetUrl(options.assets?.ppt?.wasmUrl, 'ppt/ppt_view_rs_bg.wasm', bundledWasm).href;
  const workerAdapter = createWorkerAdapter<PptDoc>({
    format: 'pptx',
    formats: ['pptx'],
    workerData: { wasmUrl },
    createWorker: () => options.workerFactory
      ? options.workerFactory(workerUrl, { type: 'module' })
      : new Worker(workerUrl, { type: 'module' }),
  });

  return {
    async open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: PptDoc }> {
      const result = await workerAdapter.open(request);
      return {
        format: request.format ?? 'pptx',
        durationMs: result.durationMs,
        stats: { slides: result.document.slides.length },
        pageCount: result.document.slides.length,
        warnings: result.warnings.concat((result.document.warnings ?? []).map(message => ({ code: 'DOCUMENT_WARNING', message }))),
        document: result.document,
      };
    },
    render(host: HTMLElement, document: PptDoc) {
      const rendered = renderPptInto(host, document);
      return { ...rendered, stats: { ...rendered.stats } };
    },
    destroy: workerAdapter.destroy,
  };
}
