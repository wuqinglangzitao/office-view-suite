export type OfficeFormat = 'doc' | 'docx' | 'docm' | 'pptx' | 'xlsx' | 'xlsm' | 'pdf';

export interface OfficeSource {
  bytes: Blob | ArrayBuffer | Uint8Array;
  name?: string;
  format?: OfficeFormat;
}

export interface OfficeOpenRequest extends OfficeSource {
  signal?: AbortSignal;
}

export interface OfficeAssetOptions {
  /** Directory containing the word/, excel/, ppt/, and pdf/ asset directories. */
  baseUrl?: string | URL;
  word?: Partial<{ workerUrl: string; wasmUrl: string }>;
  excel?: Partial<{ workerUrl: string; wasmUrl: string }>;
  ppt?: Partial<{ workerUrl: string; wasmUrl: string }>;
  pdf?: Partial<{
    workerUrl: string;
    pdfiumJsUrl: string;
    pdfiumWasmUrl: string;
    glueJsUrl: string;
    glueWasmUrl: string;
  }>;
}

export interface OfficeWorkerFactory {
  (url: URL, options?: WorkerOptions): Worker;
}

export interface OfficeViewerOptions {
  format?: OfficeFormat;
  assets?: OfficeAssetOptions;
  workerFactory?: OfficeWorkerFactory;
}

export interface OfficeViewerError extends Error {
  code:
    | 'UNSUPPORTED_FORMAT'
    | 'INVALID_SOURCE'
    | 'ASSET_LOAD_FAILED'
    | 'PARSE_FAILED'
    | 'RENDER_FAILED'
    | 'ABORTED'
    | 'DESTROYED';
}

export interface OfficeWarning {
  code: string;
  message: string;
}

export interface OfficeOpenResult {
  format: OfficeFormat;
  pageCount?: number;
  itemCount?: number;
  durationMs: number;
  /** Additional format-specific counts, retained for the demo applications. */
  stats: Record<string, number>;
  warnings: OfficeWarning[];
}

export type OfficeViewerEvent =
  | { type: 'statechange'; state: OfficeViewerState }
  | { type: 'warning'; code: string; message: string }
  | { type: 'error'; error: OfficeViewerError };

export type OfficeViewerListener = (event: OfficeViewerEvent) => void;
export type OfficeViewerState = 'idle' | 'loading' | 'ready' | 'error' | 'destroyed';

export type OfficeZoom = number | 'fit-width';
export interface OfficeZoomState { mode: 'fixed' | 'fit-width'; scale: number }
export interface OfficeViewerCapabilities { readonly zoom: boolean; readonly fitWidth: boolean }

export interface OfficeViewer {
  open(request: OfficeOpenRequest): Promise<OfficeOpenResult>;
  clear(): void;
  resize(): void;
  /** Fixed scale from 0.1 to 4, or fit the document width to its container. */
  setZoom(zoom: OfficeZoom): void;
  getZoom(): OfficeZoomState;
  getCapabilities(): OfficeViewerCapabilities;
  destroy(): void;
  on(listener: OfficeViewerListener): () => void;
}

export interface WorkerResponse<TDocument> {
  id: number;
  ok: true;
  durationMs: number;
  document: TDocument;
  warnings?: Array<string | OfficeWarning>;
}

export interface WorkerErrorResponse {
  id: number;
  ok: false;
  message: string;
  code?: OfficeViewerError['code'];
}

interface WorkerOpenResult<TDocument> {
  document: TDocument;
  durationMs: number;
  warnings: OfficeWarning[];
}

export interface WorkerAdapter<TDocument> {
  open(request: OfficeOpenRequest): Promise<WorkerOpenResult<TDocument>>;
  destroy(): void;
}

export interface WorkerAdapterOptions {
  format: OfficeFormat;
  formats?: OfficeFormat[];
  createWorker: () => Worker;
  workerData?: Record<string, unknown>;
  validateName?: (name: string) => boolean;
  invalidNameMessage?: string;
}

export function createOfficeViewerError(
  code: OfficeViewerError['code'],
  message: string,
): OfficeViewerError {
  const error = new Error(message) as OfficeViewerError;
  error.name = 'OfficeViewerError';
  error.code = code;
  return error;
}

export function normalizeOfficeWarnings(warnings: Array<string | OfficeWarning> = []): OfficeWarning[] {
  return warnings.map(warning => typeof warning === 'string'
    ? { code: 'DOCUMENT_WARNING', message: warning }
    : { code: warning.code, message: warning.message });
}

const formats: readonly string[] = ['doc', 'docx', 'docm', 'pptx', 'xlsx', 'xlsm', 'pdf'];

export function inferOfficeFormat(name?: string, explicit?: OfficeFormat): OfficeFormat | undefined {
  if (explicit !== undefined) return formats.includes(explicit) ? explicit : undefined;
  const extension = name?.toLowerCase().split('.').pop();
  return extension && formats.includes(extension) ? extension as OfficeFormat : undefined;
}

interface FormatInspection { format?: OfficeFormat; invalid?: string }

/** Container inspection only: no decompression, CRC checks, or PDF object parsing. */
function inspectOfficeFormat(source: ArrayBuffer | Uint8Array): FormatInspection {
  const bytes = source instanceof Uint8Array ? source : new Uint8Array(source);
  const ole = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
  if (ole.every((value, index) => bytes[index] === value)) {
    if (bytes.length < 512) return { invalid: 'OLE 文件头不完整' };
    return new TextDecoder('utf-16le').decode(bytes).includes('WordDocument\0') ? { format: 'doc' } : {};
  }
  const zip = bytes[0] === 0x50 && bytes[1] === 0x4b;
  const zipSignature = zip && ((bytes[2] === 3 && bytes[3] === 4)
    || (bytes[2] === 1 && bytes[3] === 2) || (bytes[2] === 5 && bytes[3] === 6));
  // Strong container signatures take precedence over PDF text embedded in a
  // ZIP member name or an OLE stream. Keep PDF's tolerated leading prefix.
  if (!zipSignature && new TextDecoder().decode(bytes.subarray(0, 1024)).includes('%PDF-')) return { format: 'pdf' };
  if (bytes.length < 22) return zip ? { invalid: 'ZIP 文件尾不完整' } : {};
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let end = -1;
  for (let offset = bytes.length - 22; offset >= Math.max(0, bytes.length - 65557); offset--) {
    if (view.getUint32(offset, true) === 0x06054b50 && offset + 22 + view.getUint16(offset + 20, true) === bytes.length) {
      end = offset;
      break;
    }
  }
  if (end < 0) return zip ? { invalid: 'ZIP 中央目录缺失或已截断' } : {};
  const damaged = (): FormatInspection => ({ invalid: 'ZIP 中央目录边界无效' });
  let offset = view.getUint32(end + 16, true);
  let count = view.getUint16(end + 10, true);
  let size = view.getUint32(end + 12, true);
  let directoryLimit = end;
  if (offset === 0xffffffff || size === 0xffffffff || count === 0xffff) {
    // ZIP64 uses a locator immediately before the ordinary end record.
    const locator = end - 20;
    if (locator < 0 || view.getUint32(locator, true) !== 0x07064b50) return damaged();
    const safe64 = (at: number): number => {
      const value = view.getBigUint64(at, true);
      return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : -1;
    };
    const wide = safe64(locator + 8);
    if (wide < 0 || wide + 56 > locator || view.getUint32(wide, true) !== 0x06064b50) return damaged();
    const recordSize = safe64(wide + 4);
    if (recordSize < 44 || wide + 12 + recordSize > locator) return damaged();
    count = safe64(wide + 32);
    size = safe64(wide + 40);
    offset = safe64(wide + 48);
    directoryLimit = wide;
  }
  if (count < 0 || size < 0 || offset < 0 || offset + size > directoryLimit || count > size / 46) return damaged();
  const directoryEnd = offset + size;
  const names = new Set<string>();
  for (let index = 0; index < count; index++) {
    if (offset + 46 > directoryEnd || view.getUint32(offset, true) !== 0x02014b50) return damaged();
    const nameLength = view.getUint16(offset + 28, true);
    const next = offset + 46 + nameLength + view.getUint16(offset + 30, true) + view.getUint16(offset + 32, true);
    if (next > directoryEnd) return damaged();
    names.add(new TextDecoder().decode(bytes.subarray(offset + 46, offset + 46 + nameLength)));
    offset = next;
  }
  if (names.has('word/document.xml')) return { format: names.has('word/vbaProject.bin') ? 'docm' : 'docx' };
  if (names.has('xl/workbook.xml')) return { format: names.has('xl/vbaProject.bin') ? 'xlsm' : 'xlsx' };
  if (names.has('ppt/presentation.xml')) return { format: 'pptx' };
  return { invalid: 'ZIP 中未找到支持的 Office 文档主条目' };
}

/** Non-throwing format probe. Definite container errors are reported by resolveOfficeFormat. */
export function detectOfficeFormat(source: ArrayBuffer | Uint8Array): OfficeFormat | undefined {
  return inspectOfficeFormat(source).format;
}

function sameFormatFamily(left: OfficeFormat, right: OfficeFormat): boolean {
  return left === right
    || (['docx', 'docm'].includes(left) && ['docx', 'docm'].includes(right))
    || (['xlsx', 'xlsm'].includes(left) && ['xlsx', 'xlsm'].includes(right));
}

/** Content chooses the renderer; a macro-enabled name may refine its OOXML family. */
export function resolveOfficeFormat(
  source: ArrayBuffer | Uint8Array,
  hint: Pick<OfficeSource, 'name' | 'format'> = {},
): OfficeFormat {
  if (hint.format !== undefined && !inferOfficeFormat(undefined, hint.format)) {
    throw createOfficeViewerError('UNSUPPORTED_FORMAT', '不支持此文件格式');
  }
  const inspected = inspectOfficeFormat(source);
  if (inspected.invalid) throw createOfficeViewerError('INVALID_SOURCE', inspected.invalid);
  const detected = inspected.format;
  if (!detected) {
    throw createOfficeViewerError(inferOfficeFormat(hint.name, hint.format) ? 'INVALID_SOURCE' : 'UNSUPPORTED_FORMAT', '无法从文件内容识别支持的文档格式');
  }
  if (hint.format && !sameFormatFamily(hint.format, detected)) {
    throw createOfficeViewerError('INVALID_SOURCE', `指定格式 ${hint.format} 与文件内容 ${detected} 不匹配`);
  }
  const named = inferOfficeFormat(hint.name);
  return hint.format ?? (named?.endsWith('m') && sameFormatFamily(named, detected) ? named : detected);
}

/** Always return owned bytes, so transferring to a Worker never detaches the caller's input. */
export async function copyOfficeBytes(source: OfficeSource['bytes']): Promise<ArrayBuffer> {
  try {
    let copy: ArrayBuffer;
    if (typeof Blob !== 'undefined' && source instanceof Blob) copy = await source.arrayBuffer();
    else if (source instanceof ArrayBuffer) copy = source.slice(0);
    else if (source instanceof Uint8Array) copy = new Uint8Array(source).buffer;
    else throw new Error('bytes 必须是 Blob、ArrayBuffer 或 Uint8Array');
    if (copy.byteLength === 0) throw new Error('文件内容不能为空');
    return copy;
  } catch (error) {
    throw createOfficeViewerError('INVALID_SOURCE', error instanceof Error ? error.message : String(error));
  }
}

interface OwnedOfficeInput {
  bytes: ArrayBuffer;
  format: OfficeFormat;
}

const preparedRequests = new WeakMap<OfficeOpenRequest, OwnedOfficeInput>();

/** @internal Copy and inspect once before routing through the facade. */
export async function prepareOfficeRequest(request: OfficeOpenRequest): Promise<OfficeOpenRequest & OwnedOfficeInput> {
  const { name, format, signal } = request;
  if (signal?.aborted) throw createOfficeViewerError('ABORTED', '打开操作已取消');
  const bytes = await copyOfficeBytes(request.bytes);
  if (signal?.aborted) throw createOfficeViewerError('ABORTED', '打开操作已取消');
  const input = { bytes, format: resolveOfficeFormat(bytes, { name, format }) };
  const prepared = Object.freeze({ ...input, name, signal });
  preparedRequests.set(prepared, input);
  return prepared;
}

/** @internal Claim prepared input once; otherwise inspect the copy after checking cancellation. */
export async function takeOfficeInput(request: OfficeOpenRequest): Promise<{ bytes: ArrayBuffer; format?: OfficeFormat }> {
  const prepared = preparedRequests.get(request);
  if (prepared) {
    preparedRequests.delete(request);
    return prepared;
  }
  const bytes = await copyOfficeBytes(request.bytes);
  return { bytes };
}

export function createWorkerAdapter<TDocument>(options: WorkerAdapterOptions): WorkerAdapter<TDocument> {
  type Pending = {
    id: number;
    posted: boolean;
    resolve(value: WorkerOpenResult<TDocument>): void;
    reject(error: OfficeViewerError): void;
    cleanup(): void;
  };
  let worker: Worker | undefined;
  let workerResponded = false;
  let nextId = 1;
  let destroyed = false;
  let pending: Pending | undefined;

  const stopWorker = (): void => {
    if (!worker) return;
    worker.onmessage = null;
    worker.onerror = null;
    worker.onmessageerror = null;
    worker.terminate();
    worker = undefined;
    workerResponded = false;
  };

  const rejectPending = (error: OfficeViewerError, terminate: boolean): void => {
    const request = pending;
    pending = undefined;
    if (terminate) stopWorker();
    request?.cleanup();
    request?.reject(error);
  };

  const getWorker = (): Worker => {
    if (worker) return worker;
    const created = options.createWorker();
    worker = created;
    workerResponded = false;
    created.onmessage = (event: MessageEvent<WorkerResponse<TDocument> | WorkerErrorResponse>) => {
      if (worker !== created) return;
      const response = event.data;
      const request = pending;
      if (!request || !response || response.id !== request.id) return;
      workerResponded = true;
      pending = undefined;
      request.cleanup();
      if (response.ok) request.resolve({
        document: response.document,
        durationMs: response.durationMs,
        warnings: normalizeOfficeWarnings(response.warnings),
      });
      else request.reject(createOfficeViewerError(response.code ?? 'PARSE_FAILED', response.message));
    };
    created.onerror = event => {
      if (worker !== created) return;
      event.preventDefault?.();
      rejectPending(createOfficeViewerError(workerResponded ? 'PARSE_FAILED' : 'ASSET_LOAD_FAILED', event.message || 'Worker 加载或执行失败'), true);
    };
    created.onmessageerror = () => {
      if (worker === created) rejectPending(createOfficeViewerError('PARSE_FAILED', '无法读取 Worker 响应'), true);
    };
    return created;
  };

  return {
    open(request): Promise<WorkerOpenResult<TDocument>> {
      if (destroyed) return Promise.reject(createOfficeViewerError('DESTROYED', 'viewer 已销毁'));
      if (request.signal?.aborted) return Promise.reject(createOfficeViewerError('ABORTED', '打开操作已取消'));
      const explicit = request.format;
      const format = explicit === undefined ? undefined : inferOfficeFormat(undefined, explicit);
      const allowed = options.formats ?? [options.format];
      if ((explicit !== undefined && !format) || (format && !allowed.includes(format))) {
        return Promise.reject(createOfficeViewerError('UNSUPPORTED_FORMAT', options.invalidNameMessage ?? '不支持此文件格式'));
      }
      rejectPending(createOfficeViewerError('ABORTED', '打开操作已被新的请求替代'), Boolean(pending?.posted));
      return new Promise((resolve, reject) => {
        const id = nextId++;
        const onAbort = () => {
          if (pending?.id === id) rejectPending(createOfficeViewerError('ABORTED', '打开操作已取消'), pending.posted);
        };
        pending = { id, posted: false, resolve, reject, cleanup: () => request.signal?.removeEventListener('abort', onAbort) };
        request.signal?.addEventListener('abort', onAbort, { once: true });
        void takeOfficeInput(request).then(({ bytes, format }) => {
          // A Blob may finish reading after abort, replacement, or destruction.
          if (destroyed || pending?.id !== id) return;
          const resolvedFormat = format ?? resolveOfficeFormat(bytes, request);
          if (!allowed.includes(resolvedFormat)) {
            rejectPending(createOfficeViewerError('UNSUPPORTED_FORMAT', options.invalidNameMessage ?? '无法识别文件格式'), false);
            return;
          }
          let target: Worker;
          try {
            target = getWorker();
          } catch (error) {
            rejectPending(createOfficeViewerError('ASSET_LOAD_FAILED', error instanceof Error ? error.message : String(error)), true);
            return;
          }
          try {
            pending.posted = true;
            target.postMessage({ ...options.workerData, id, name: request.name ?? `document.${resolvedFormat}`, format: resolvedFormat, bytes }, [bytes]);
          } catch (error) {
            rejectPending(createOfficeViewerError('PARSE_FAILED', error instanceof Error ? error.message : String(error)), true);
          }
        }).catch(error => {
          if (pending?.id === id) rejectPending(error, false);
        });
      });
    },
    destroy(): void {
      if (destroyed) return;
      destroyed = true;
      rejectPending(createOfficeViewerError('DESTROYED', 'viewer 已销毁'), true);
    },
  };
}
