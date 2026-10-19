import {
  takeOfficeInput,
  createOfficeViewerError,
  resolveOfficeFormat,
  type OfficeAssetOptions,
  type OfficeOpenRequest,
  type OfficeOpenResult,
  type OfficeViewerError,
  type OfficeViewerOptions,
} from '@office-view-suite/viewer-core';

interface PdfPageSize { wPt: number; hPt: number }
interface PdfDocument { pages: PdfPageSize[]; session: number }
interface PdfMetaMessage { type: 'meta'; id: number; session: number; pages: PdfPageSize[] }
interface PdfPageMessage { type: 'page'; id: number; session: number; page: number; image: ImageData }
interface PdfErrorMessage {
  type: 'error'; id: number; session: number;
  code: OfficeViewerError['code']; message: string;
}
type PdfMessage = PdfMetaMessage | PdfPageMessage | PdfErrorMessage;
interface Session {
  id: number;
  error?: OfficeViewerError;
  cancelled: Promise<never>;
  cancel(error: OfficeViewerError): void;
}
interface Pending {
  session: Session;
  type: 'open' | 'render';
  resolve(message: PdfMessage): void;
  reject(error: OfficeViewerError): void;
}
interface PdfRendered {
  stats: Record<string, number>;
  dispose(): void;
  resize(): void;
}

export interface PdfAssetOptions {
  workerUrl: string;
  pdfiumJsUrl: string;
  pdfiumWasmUrl: string;
  glueJsUrl: string;
  glueWasmUrl: string;
}

export interface PdfAdapter {
  open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: PdfDocument }>;
  render(host: HTMLElement, document: PdfDocument, onError?: (error: OfficeViewerError) => void): PdfRendered;
  clear(): void;
  destroy(): void;
}

const SHADOW_CSS = `
:host { display: block; min-width: 0; }
.pages { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 16px; box-sizing: border-box; }
.page { display: block; max-width: 100%; background: #fff; box-shadow: 0 2px 8px rgba(0,0,0,.35); border-radius: 2px; }
`;

export function createPdfAdapter(options: OfficeViewerOptions = {}): PdfAdapter {
  const assets = resolveAssets(options.assets?.baseUrl, options.assets?.pdf);
  let worker: Worker | undefined;
  let destroyed = false;
  let nextId = 1;
  let nextSession = 1;
  let current: Session | undefined;
  let rendered: PdfRendered | undefined;
  let hasCancelledRenders = false;
  const pending = new Map<number, Pending>();

  function stopWorker(): void {
    if (!worker) return;
    worker.onmessage = null;
    worker.onerror = null;
    worker.onmessageerror = null;
    worker.terminate();
    worker = undefined;
    hasCancelledRenders = false;
  }

  function invalidate(error: OfficeViewerError): void {
    const inFlight = pending.size > 0 || hasCancelledRenders;
    rendered?.dispose();
    rendered = undefined;
    if (current) {
      current.error = error;
      current.cancel(error);
      current = undefined;
    }
    // PDFium executes synchronously. Terminating cancels work already inside
    // WASM; an idle initialized worker can be reused by the next document.
    if (inFlight) stopWorker();
    for (const request of pending.values()) request.reject(error);
    pending.clear();
  }

  function assertCurrent(session: Session): void {
    if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
    if (session.error) throw session.error;
    if (current !== session) throw createOfficeViewerError('ABORTED', '打开操作已取消');
  }

  function ensureWorker(): Worker {
    if (worker) return worker;
    const url = new URL(assets.workerUrl);
    try {
      worker = options.workerFactory
        ? options.workerFactory(url, { type: 'classic' })
        : new Worker(url, { type: 'classic' });
    } catch (error) {
      throw createOfficeViewerError('ASSET_LOAD_FAILED', errorMessage(error));
    }
    const createdWorker = worker;
    worker.onmessage = (event: MessageEvent<PdfMessage>) => {
      if (worker !== createdWorker) return;
      const message = event.data;
      const request = message && pending.get(message.id);
      if (!request || message.session !== request.session.id) return;
      pending.delete(message.id);
      if (message.type === 'error') {
        request.reject(createOfficeViewerError(message.code ?? (request.type === 'open' ? 'PARSE_FAILED' : 'RENDER_FAILED'), message.message));
      } else {
        request.resolve(message);
      }
    };
    worker.onerror = event => {
      if (worker !== createdWorker) return;
      event.preventDefault?.();
      const error = createOfficeViewerError('ASSET_LOAD_FAILED', event.message || 'PDF Worker 加载或执行失败');
      if (current) current.error = error;
      stopWorker();
      for (const request of pending.values()) request.reject(error);
      pending.clear();
    };
    worker.onmessageerror = () => {
      if (worker !== createdWorker) return;
      stopWorker();
      for (const request of pending.values()) {
        const error = createOfficeViewerError(request.type === 'open' ? 'PARSE_FAILED' : 'RENDER_FAILED', '无法读取 PDF Worker 响应');
        if (current) current.error = error;
        request.reject(error);
      }
      pending.clear();
    };
    return worker;
  }

  function send(session: Session, type: 'open' | 'render', data: Record<string, unknown>, transfer: Transferable[] = []): Promise<PdfMessage> {
    return new Promise((resolve, reject) => {
      try {
        assertCurrent(session);
        const target = ensureWorker();
        const id = nextId++;
        pending.set(id, { session, type, resolve, reject });
        try {
          target.postMessage({ ...data, type, id, session: session.id, assets }, transfer);
        } catch (error) {
          pending.delete(id);
          reject(createOfficeViewerError(type === 'open' ? 'PARSE_FAILED' : 'RENDER_FAILED', errorMessage(error)));
        }
      } catch (error) {
        reject(error);
      }
    });
  }

  return {
    async open(request) {
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      invalidate(createOfficeViewerError('ABORTED', '已被新的打开操作替代'));
      if (request.signal?.aborted) throw createOfficeViewerError('ABORTED', '打开操作已取消');
      if ((request.format ?? options.format) && (request.format ?? options.format) !== 'pdf') {
        throw createOfficeViewerError('UNSUPPORTED_FORMAT', 'PDF adapter 仅支持 PDF 文件');
      }
      const started = performance.now();
      let cancel!: Session['cancel'];
      const cancelled = new Promise<never>((_, reject) => { cancel = reject; });
      const session: Session = { id: nextSession++, cancelled, cancel };
      current = session;
      const abort = () => {
        if (current === session) invalidate(createOfficeViewerError('ABORTED', '打开操作已取消'));
      };
      request.signal?.addEventListener('abort', abort, { once: true });
      try {
        const input = await Promise.race([takeOfficeInput(request), cancelled]);
        assertCurrent(session);
        const { bytes } = input;
        const format = input.format ?? resolveOfficeFormat(bytes, { name: request.name, format: request.format ?? options.format });
        if (format !== 'pdf') throw createOfficeViewerError('UNSUPPORTED_FORMAT', 'PDF adapter 仅支持 PDF 文件');
        const response = await send(session, 'open', { bytes }, [bytes]);
        assertCurrent(session);
        if (response.type !== 'meta' || !Array.isArray(response.pages) || response.pages.some(page =>
          !Number.isFinite(page.wPt) || !Number.isFinite(page.hPt) || page.wPt <= 0 || page.hPt <= 0)) {
          throw createOfficeViewerError('PARSE_FAILED', 'PDF Worker 返回了无效页面尺寸');
        }
        return {
          format: 'pdf', pageCount: response.pages.length, itemCount: response.pages.length,
          durationMs: performance.now() - started,
          stats: { pages: response.pages.length }, warnings: [],
          document: { pages: response.pages, session: session.id },
        };
      } catch (error) {
        if (current === session) {
          current = undefined;
          stopWorker();
        }
        throw error;
      } finally {
        request.signal?.removeEventListener('abort', abort);
      }
    },
    render(host, doc, onError) {
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      const session = current;
      if (!session || doc.session !== session.id) throw createOfficeViewerError('ABORTED', 'PDF 文档已失效');
      rendered?.dispose();
      const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
      shadow.replaceChildren();
      const style = document.createElement('style');
      style.textContent = SHADOW_CSS;
      const pages = document.createElement('div');
      pages.className = 'pages';
      shadow.append(style, pages);
      let disposed = false;
      let frame = 0;
      let errorReported = false;
      const pageStates = doc.pages.map((size, page) => {
        const canvas = document.createElement('canvas');
        canvas.width = 0;
        canvas.height = 0;
        canvas.className = 'page';
        canvas.dataset.page = String(page);
        canvas.setAttribute('aria-label', `第 ${page + 1} 页`);
        pages.appendChild(canvas);
        return { size, page, canvas, visible: false, wantedWidth: 0, paintedWidth: 0, rendering: false };
      });
      type PageState = typeof pageStates[number];
      const alive = () => !disposed && !destroyed && current === session;

      const releaseBitmap = (state: PageState): void => {
        // Explicit CSS dimensions keep the scroll layout stable without pixels.
        state.canvas.width = 0;
        state.canvas.height = 0;
        state.paintedWidth = 0;
      };

      async function paint(state: PageState): Promise<void> {
        if (!alive() || state.rendering || !state.visible || state.paintedWidth === state.wantedWidth) return;
        state.rendering = true;
        const width = state.wantedWidth;
        try {
          const response = await send(session!, 'render', { page: state.page, width });
          if (!alive() || !state.visible) return;
          if (response.type !== 'page' || response.page !== state.page) {
            throw createOfficeViewerError('RENDER_FAILED', 'PDF 页面渲染响应无效');
          }
          // A resize can supersede a render without replacing the page element.
          if (width === state.wantedWidth) {
            state.canvas.width = response.image.width;
            state.canvas.height = response.image.height;
            const context = state.canvas.getContext('2d');
            if (!context) throw createOfficeViewerError('RENDER_FAILED', '无法创建 PDF Canvas 上下文');
            context.putImageData(response.image, 0, 0);
            state.paintedWidth = width;
            delete state.canvas.dataset.error;
          }
        } catch (error) {
          if (!alive()) return;
          // Mark this size as attempted so failed pages do not retry forever.
          state.paintedWidth = state.visible ? width : 0;
          state.canvas.dataset.error = errorMessage(error);
          if (!errorReported) {
            errorReported = true;
            const failure = error instanceof Error && 'code' in error
              ? error as OfficeViewerError
              : createOfficeViewerError('RENDER_FAILED', errorMessage(error));
            try { onError?.(failure); } catch { /* Host callbacks must not reject background work. */ }
          }
        } finally {
          state.rendering = false;
          if (alive() && state.wantedWidth !== width) void paint(state);
        }
      }

      const layout = () => {
        if (!alive()) return;
        const logicalWidth = Math.max(1, Math.min((pages.clientWidth || host.clientWidth || 932) - 32, 900));
        const pixelRatio = Math.max(1, Math.min(globalThis.devicePixelRatio || 1, 3));
        for (const state of pageStates) {
          state.canvas.style.width = `${logicalWidth}px`;
          state.canvas.style.height = `${state.size.hPt / state.size.wPt * logicalWidth}px`;
          state.wantedWidth = Math.max(1, Math.round(logicalWidth * pixelRatio));
          if (state.visible) void paint(state);
        }
      };
      const observer = typeof IntersectionObserver === 'undefined' ? undefined : new IntersectionObserver(entries => {
        for (const entry of entries) {
          const state = pageStates[Number((entry.target as HTMLCanvasElement).dataset.page)];
          if (!state) continue;
          state.visible = entry.isIntersecting;
          if (state.visible) void paint(state);
          else releaseBitmap(state);
        }
      }, { rootMargin: '300px' });
      layout();
      for (const state of pageStates) {
        if (observer) observer.observe(state.canvas);
        else { state.visible = true; void paint(state); }
      }
      const handle: PdfRendered = {
        stats: { pages: doc.pages.length },
        resize() {
          if (!alive() || frame) return;
          frame = requestAnimationFrame(() => { frame = 0; layout(); });
        },
        dispose() {
          if (disposed) return;
          disposed = true;
          observer?.disconnect();
          if (frame) cancelAnimationFrame(frame);
          for (const state of pageStates) releaseBitmap(state);
          for (const [id, request] of pending) {
            if (request.session === session && request.type === 'render') {
              hasCancelledRenders = true;
              pending.delete(id);
              request.reject(createOfficeViewerError('ABORTED', 'PDF 渲染已取消'));
            }
          }
          // Remove only this render's elements; a later render may share the root.
          style.remove();
          pages.remove();
        },
      };
      rendered = handle;
      return handle;
    },
    clear() { invalidate(createOfficeViewerError('ABORTED', '预览已清空')); },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      invalidate(createOfficeViewerError('DESTROYED', 'viewer 已销毁'));
      stopWorker();
    },
  };
}

function errorMessage(error: unknown): string { return error instanceof Error ? error.message : String(error); }

function resolveAssets(baseUrl?: string | URL, overrides?: OfficeAssetOptions['pdf']): PdfAssetOptions {
  const documentBase = typeof document === 'undefined' ? import.meta.url : document.baseURI;
  const base = baseUrl ? new URL(String(baseUrl), documentBase) : undefined;
  if (base && !base.pathname.endsWith('/')) base.pathname += '/';
  const path = (name: string, bundled: URL, override?: string) =>
    override ? new URL(override, documentBase).href : base ? new URL(`pdf/${name}`, base).href : bundled.href;
  return {
    workerUrl: path('worker.js', new URL('./assets/pdf/worker.js?no-inline', import.meta.url), overrides?.workerUrl),
    pdfiumJsUrl: path('pdfium/pdfium.std.js', new URL('./assets/pdf/pdfium/pdfium.std.js?no-inline', import.meta.url), overrides?.pdfiumJsUrl),
    pdfiumWasmUrl: path('pdfium/pdfium.std.wasm', new URL('./assets/pdf/pdfium/pdfium.std.wasm?no-inline', import.meta.url), overrides?.pdfiumWasmUrl),
    glueJsUrl: path('glue/pdfium_view_rs.js', new URL('./assets/pdf/glue/pdfium_view_rs.js?no-inline', import.meta.url), overrides?.glueJsUrl),
    glueWasmUrl: path('glue/pdfium_view_rs_bg.wasm', new URL('./assets/pdf/glue/pdfium_view_rs_bg.wasm?no-inline', import.meta.url), overrides?.glueWasmUrl),
  };
}
