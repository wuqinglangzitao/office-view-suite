import {
  copyOfficeBytes,
  createOfficeViewerError,
  detectOfficeFormat,
  inferOfficeFormat,
  type OfficeFormat,
  type OfficeOpenRequest,
  type OfficeOpenResult,
  type OfficeViewer,
  type OfficeViewerError,
  type OfficeViewerEvent,
  type OfficeViewerListener,
  type OfficeViewerOptions,
  type OfficeViewerState,
} from '@office-view-suite/viewer-core';

type Rendered = { stats: Record<string, number>; dispose(): void; resize?(): void };
type RuntimeAdapter = {
  open(request: OfficeOpenRequest): Promise<OfficeOpenResult & { document: unknown }>;
  render(host: HTMLElement, document: unknown, onError?: (error: OfficeViewerError) => void): Rendered;
  clear?(): void;
  destroy(): void;
};
type AdapterKind = 'word' | 'excel' | 'ppt' | 'pdf';

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
  const host = container.ownerDocument.createElement('div');
  host.className = 'office-viewer-host';
  host.style.display = 'block';
  host.style.width = '100%';
  host.style.minWidth = '0';
  host.attachShadow({ mode: 'open' });
  container.appendChild(host);

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
    if (kind === 'pdf') {
      const module = await import('@office-view-suite/viewer-pdf');
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      return module.createPdfAdapter(options);
    }
    if (kind === 'ppt') {
      const module = await import('@office-view-suite/viewer-ppt');
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      return module.createPptAdapter(options);
    }
    if (kind === 'excel') {
      const module = await import('@office-view-suite/viewer-excel');
      if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
      return module.createExcelAdapter(options);
    }
    const module = await import('@office-view-suite/viewer-word');
    if (destroyed) throw createOfficeViewerError('DESTROYED', 'viewer 已销毁');
    return module.createWordAdapter(options);
  }

  const getAdapter = (format: OfficeFormat): Promise<RuntimeAdapter> => {
    const kind: AdapterKind = format === 'pdf' ? 'pdf' : format === 'pptx' ? 'ppt'
      : format === 'xlsx' || format === 'xlsm' ? 'excel' : 'word';
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
      });
      lastWidth = container.getBoundingClientRect().width;
    } catch (error) {
      throw asViewerError(error, 'RENDER_FAILED');
    }
  };

  const resizeCurrent = (): void => {
    if (destroyed || state !== 'ready') return;
    try {
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
        const bytes = await untilAborted(copyOfficeBytes(request.bytes), session.signal);
        const format = inferOfficeFormat(request.name, explicit) ?? detectOfficeFormat(bytes);
        if (!format) throw createOfficeViewerError('UNSUPPORTED_FORMAT', '无法从文件名或文件内容识别格式，请指定 format');
        const adapter = await untilAborted(getAdapter(format), session.signal);
        if (session.signal.aborted) throw abortError(session.signal);
        const result = await untilAborted(adapter.open({ ...request, bytes, format, signal: session.signal }), session.signal);
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
      host.remove();
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
} from '@office-view-suite/viewer-core';
