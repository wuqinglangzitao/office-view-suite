import {
  createOfficeViewerError,
  inferOfficeFormat,
  prepareOfficeRequest,
  type OfficeFormat,
  type OfficeOpenRequest,
  type OfficeOpenResult,
  type OfficeViewer,
  type OfficeViewerError,
  type OfficeViewerEvent,
  type OfficeViewerListener,
  type OfficeViewerOptions,
  type OfficeViewerState,
  type OfficeZoom,
} from '@office-view-suite/viewer-core';

type Rendered = { stats: Record<string, number>; dispose(): void; resize?(): void; getContentWidth?(): number };
type RuntimeAdapter = {
  open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: unknown }>;
  render(host: HTMLElement, document: unknown, onError?: (error: OfficeViewerError) => void, onContentChange?: () => void): Rendered;
  clear?(): void;
  destroy(): void;
};
type AdapterKind = 'word' | 'excel' | 'ppt' | 'pdf';

const adapterKinds: Record<OfficeFormat, AdapterKind> = {
  doc: 'word', docx: 'word', docm: 'word', xlsx: 'excel', xlsm: 'excel', pptx: 'ppt', pdf: 'pdf',
};
const adapterLoaders = {
  word: async () => (await import('@office-view-suite/viewer-word')).createWordAdapter,
  excel: async () => (await import('@office-view-suite/viewer-excel')).createExcelAdapter,
  ppt: async () => (await import('@office-view-suite/viewer-ppt')).createPptAdapter,
  pdf: async () => (await import('@office-view-suite/viewer-pdf')).createPdfAdapter,
};

function asViewerError(error: unknown, code: OfficeViewerError['code']): OfficeViewerError {
  return error instanceof Error && 'code' in error
    ? error as OfficeViewerError
    : createOfficeViewerError(code, error instanceof Error ? error.message : String(error));
}

function abortError(signal: AbortSignal): OfficeViewerError {
  return signal.reason?.code === 'DESTROYED'
    ? signal.reason as OfficeViewerError
    : createOfficeViewerError('ABORTED', '打开操作已取消');
}

/** Observe cancellation even while importing an adapter or reading a Blob. */
function untilAborted<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const onAbort = () => reject(abortError(signal));
    if (signal.aborted) onAbort();
    else signal.addEventListener('abort', onAbort, { once: true });
    promise.then(resolve, reject).finally(() => signal.removeEventListener('abort', onAbort));
  });
}

export function createOfficeViewer(container: HTMLElement, options: OfficeViewerOptions = {}): OfficeViewer {
  // The caller owns its container, existing children, style, and possible shadow root.
  const viewport = container.ownerDocument.createElement('div');
  viewport.className = 'office-viewer-host';
  viewport.style.width = '100%';
  viewport.style.minWidth = '0';
  viewport.style.overflow = 'auto';
  const host = container.ownerDocument.createElement('div');
  host.className = 'office-viewer-surface';
  host.style.display = 'block';
  host.style.width = '100%';
  host.style.minWidth = '0';
  host.attachShadow({ mode: 'open' });
  viewport.appendChild(host);
  container.appendChild(viewport);

  const listeners = new Set<OfficeViewerListener>();
  const adapters = new Map<AdapterKind, Promise<RuntimeAdapter>>();
  let state: OfficeViewerState = 'idle';
  let destroyed = false;
  let currentAdapter: RuntimeAdapter | undefined;
  let currentDocument: unknown;
  let currentRendered: Rendered | undefined;
  let renderGeneration = 0;
  let resizeObserver: ResizeObserver | undefined;
  let activeSession: AbortController | undefined;
  let lastWidth = container.getBoundingClientRect().width;
  let resizeFrame: number | undefined;
  let zoom: OfficeZoom = 1;
  let scale = 1;
  let customZoom = false;

  const applyZoom = (): void => {
    if (!customZoom) return;
    if (typeof zoom === 'number') scale = zoom;
    const width = viewport.clientWidth || container.getBoundingClientRect().width;
    if (width <= 0) return;
    let contentWidth = currentRendered?.getContentWidth?.() ?? host.scrollWidth ?? width;
    // Page-based renderers may center a fixed-width page outside the surface.
    // Its width is not fully represented by scrollWidth when it overflows left.
    if (!currentRendered?.getContentWidth) {
      let pageWidth = 0;
      for (const page of host.shadowRoot?.querySelectorAll<HTMLElement>('.page, .slide') ?? []) {
        const padding = page.parentElement ? getComputedStyle(page.parentElement) : undefined;
        pageWidth = Math.max(pageWidth, page.getBoundingClientRect().width / scale
          + (Number.parseFloat(padding?.paddingLeft ?? '') || 0) + (Number.parseFloat(padding?.paddingRight ?? '') || 0));
      }
      if (pageWidth > 0) contentWidth = pageWidth;
    }
    scale = zoom === 'fit-width' ? Math.min(4, Math.max(0.1, width / Math.max(1, contentWidth))) : zoom;
    host.style.zoom = String(scale);
    host.style.width = `${zoom === 'fit-width' ? width / scale : width}px`;
    host.style.setProperty('--office-viewer-viewport-width', `${width / scale}px`);
  };

  const emit = (event: OfficeViewerEvent): void => {
    if (event.type === 'statechange') state = event.state;
    for (const listener of [...listeners]) {
      // A consumer's event handler must not turn successful rendering into an SDK failure.
      try { listener(event); } catch { /* Listener errors belong to the host application. */ }
    }
  };

  const clearRendered = (): void => {
    renderGeneration++;
    const rendered = currentRendered;
    const adapter = currentAdapter;
    currentRendered = undefined;
    currentDocument = undefined;
    currentAdapter = undefined;
    rendered?.dispose();
    adapter?.clear?.();
    host.shadowRoot?.replaceChildren();
  };

  async function loadAdapter(kind: AdapterKind): Promise<RuntimeAdapter> {
    const create = await adapterLoaders[kind]();
    if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
    return create(options);
  }

  const getAdapter = (format: OfficeFormat): Promise<RuntimeAdapter> => {
    const kind = adapterKinds[format];
    const cached = adapters.get(kind);
    if (cached) return cached;
    const loaded = loadAdapter(kind).catch(error => {
      if (adapters.get(kind) === loaded) adapters.delete(kind);
      throw asViewerError(error, 'ASSET_LOAD_FAILED');
    });
    adapters.set(kind, loaded);
    return loaded;
  };

  const renderCurrent = (): void => {
    if (!currentAdapter || currentDocument === undefined || destroyed) return;
    const generation = ++renderGeneration;
    currentRendered?.dispose();
    currentRendered = undefined;
    try {
      currentRendered = currentAdapter.render(host, currentDocument, error => {
        if (destroyed || generation !== renderGeneration) return;
        emit({ type: 'statechange', state: 'error' });
        emit({ type: 'error', error: asViewerError(error, 'RENDER_FAILED') });
      }, () => {
        if (generation === renderGeneration) resizeCurrent();
      });
      applyZoom();
      if (customZoom) currentRendered.resize?.();
      lastWidth = container.getBoundingClientRect().width;
    } catch (error) {
      throw asViewerError(error, 'RENDER_FAILED');
    }
  };

  const resizeCurrent = (): void => {
    if (destroyed || state !== 'ready') return;
    try {
      applyZoom();
      if (currentRendered?.resize) currentRendered.resize();
      else renderCurrent();
      lastWidth = container.getBoundingClientRect().width;
    } catch (error) {
      emit({ type: 'statechange', state: 'error' });
      emit({ type: 'error', error: asViewerError(error, 'RENDER_FAILED') });
    }
  };

  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      // Use the same box for the initial/rendered width and observations. Mixing
      // contentRect with getBoundingClientRect loops on hosts with padding/borders.
      const width = container.getBoundingClientRect().width;
      // Rendering changes the container's height; that must never trigger a render loop.
      if (width <= 0) { lastWidth = 0; return; }
      if (Math.abs(width - lastWidth) < 0.5) return;
      lastWidth = width;
      if (state !== 'ready' || resizeFrame !== undefined) return;
      resizeFrame = requestAnimationFrame(() => {
        resizeFrame = undefined;
        resizeCurrent();
      });
    });
    resizeObserver.observe(container);
  }

  return {
    async open(request): Promise<OfficeOpenResult> {
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      if (request.signal?.aborted) throw createOfficeViewerError('ABORTED', '打开操作已取消');
      activeSession?.abort();
      const session = new AbortController();
      activeSession = session;
      const forwardAbort = () => session.abort();
      request.signal?.addEventListener('abort', forwardAbort, { once: true });
      const started = performance.now();
      try {
        clearRendered();
        emit({ type: 'statechange', state: 'loading' });
        const explicit = request.format ?? options.format;
        if (explicit !== undefined && !inferOfficeFormat(undefined, explicit)) {
          throw createOfficeViewerError('UNSUPPORTED_FORMAT', '不支持此文件格式');
        }
        const prepared = await untilAborted(prepareOfficeRequest({ ...request, format: explicit, signal: session.signal }), session.signal);
        const format = prepared.format;
        const adapter = await untilAborted(getAdapter(format), session.signal);
        if (session.signal.aborted) throw abortError(session.signal);
        const result = await untilAborted(adapter.open(prepared), session.signal);
        if (session.signal.aborted) throw abortError(session.signal);
        if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
        currentAdapter = adapter;
        currentDocument = result.document;
        renderCurrent();
        const stats = { ...result.stats, ...currentRendered?.stats };
        const merged: OfficeOpenResult = {
          format,
          durationMs: performance.now() - started,
          stats,
          pageCount: result.pageCount ?? stats.pages ?? stats.slides,
          itemCount: result.itemCount ?? stats.sheets,
          warnings: result.warnings,
        };
        emit({ type: 'statechange', state: 'ready' });
        for (const warning of merged.warnings) emit({ type: 'warning', ...warning });
        return merged;
      } catch (error) {
        const viewerError = session.signal.aborted ? abortError(session.signal) : asViewerError(error, 'PARSE_FAILED');
        if (activeSession === session && !destroyed) {
          clearRendered();
          if (viewerError.code === 'ABORTED') emit({ type: 'statechange', state: 'idle' });
          else {
            emit({ type: 'statechange', state: 'error' });
            emit({ type: 'error', error: viewerError });
          }
        }
        throw viewerError;
      } finally {
        request.signal?.removeEventListener('abort', forwardAbort);
        if (activeSession === session) activeSession = undefined;
      }
    },
    clear(): void {
      if (destroyed) return;
      activeSession?.abort();
      activeSession = undefined;
      clearRendered();
      emit({ type: 'statechange', state: 'idle' });
    },
    resize: resizeCurrent,
    setZoom(value): void {
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      if (value !== 'fit-width' && (typeof value !== 'number' || !Number.isFinite(value) || value < 0.1 || value > 4)) {
        throw createOfficeViewerError('INVALID_SOURCE', '缩放比例必须在 0.1 到 4 之间，或为 fit-width');
      }
      zoom = value;
      customZoom = true;
      if (state === 'ready') resizeCurrent();
      else applyZoom();
    },
    getZoom() { return { mode: zoom === 'fit-width' ? 'fit-width' : 'fixed', scale }; },
    getCapabilities() { return { zoom: !destroyed, fitWidth: !destroyed }; },
    destroy(): void {
      if (destroyed) return;
      destroyed = true;
      activeSession?.abort(createOfficeViewerError('DESTROYED', 'viewer 已销毁'));
      activeSession = undefined;
      clearRendered();
      resizeObserver?.disconnect();
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      for (const adapter of adapters.values()) void adapter.then(value => value.destroy(), () => {});
      adapters.clear();
      viewport.remove();
      emit({ type: 'statechange', state: 'destroyed' });
      listeners.clear();
    },
    on(listener): () => void {
      if (!destroyed) listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
  };
}

export type {
  OfficeAssetOptions,
  OfficeFormat,
  OfficeOpenRequest,
  OfficeOpenResult,
  OfficeSource,
  OfficeViewer,
  OfficeViewerError,
  OfficeViewerEvent,
  OfficeViewerListener,
  OfficeViewerOptions,
  OfficeViewerState,
  OfficeWarning,
  OfficeWorkerFactory,
  OfficeZoom,
  OfficeZoomState,
  OfficeViewerCapabilities,
} from '@office-view-suite/viewer-core';
