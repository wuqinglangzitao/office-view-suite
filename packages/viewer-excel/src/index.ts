import { createOfficeViewerError, createWorkerAdapter, type OfficeOpenRequest, type OfficeOpenResult, type OfficeViewerError, type OfficeViewerOptions } from '@office-view-suite/viewer-core';
import { renderExcelInto } from './core/render';
import type { ExcelDoc } from './core/types';

export function createExcelAdapter(options: OfficeViewerOptions = {}) {
  const bundledWorker = new URL('./assets/excel/worker.js?no-inline', import.meta.url);
  const bundledWasm = new URL('./assets/excel/excel_view_rs_bg.wasm?no-inline', import.meta.url);
  const base = options.assets?.baseUrl ? new URL(String(options.assets.baseUrl), document.baseURI) : undefined;
  if (base && !base.pathname.endsWith('/')) base.pathname += '/';
  const assetUrl = (override: string | undefined, path: string, bundled: URL): URL =>
    override ? new URL(override, document.baseURI) :
    base ? new URL(path, base) : bundled;
  const workerUrl = assetUrl(options.assets?.excel?.workerUrl, 'excel/worker.js', bundledWorker);
  const wasmUrl = assetUrl(options.assets?.excel?.wasmUrl, 'excel/excel_view_rs_bg.wasm', bundledWasm).href;
  const workerAdapter = createWorkerAdapter<ExcelDoc>({
    format: 'xlsx',
    formats: ['xlsx', 'xlsm'],
    workerData: { wasmUrl },
    createWorker: () => options.workerFactory
      ? options.workerFactory(workerUrl, { type: 'module' })
      : new Worker(workerUrl, { type: 'module' }),
  });

  return {
    async open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: ExcelDoc }> {
      const result = await workerAdapter.open(request);
      return {
        format: request.format ?? 'xlsx',
        durationMs: result.durationMs,
        stats: { sheets: result.document.sheets.length },
        itemCount: result.document.sheets.length,
        warnings: result.warnings,
        document: result.document,
      };
    },
    render(host: HTMLElement, document: ExcelDoc, onError?: (error: OfficeViewerError) => void, onContentChange?: () => void) {
      const rendered = renderExcelInto(host, document, onError ? error => {
        onError(createOfficeViewerError('RENDER_FAILED', error.message));
      } : undefined, onContentChange);
      return { ...rendered, stats: { ...rendered.stats } };
    },
    destroy: workerAdapter.destroy,
  };
}
