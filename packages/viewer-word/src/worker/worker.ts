/// <reference lib="webworker" />
import type { WordDoc } from '../core/types';

interface Request { id: number; bytes: ArrayBuffer; wasmUrl: string }
type WasmModule = typeof import('../wasm/word_view_rs');
let modulePromise: Promise<WasmModule> | undefined;
const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = async (event: MessageEvent<Request>) => {
  const req = event.data;
  let engine: WasmModule;
  try {
    modulePromise ??= import('../wasm/word_view_rs').then(async module => {
      await module.default({ module_or_path: req.wasmUrl });
      return module;
    });
    engine = await modulePromise;
  } catch (error) {
    modulePromise = undefined;
    ctx.postMessage({ id: req.id, ok: false, code: 'ASSET_LOAD_FAILED', message: String(error) });
    return;
  }
  try {
    const start = performance.now();
    const document = engine.toDocument(new Uint8Array(req.bytes)) as unknown as WordDoc;
    ctx.postMessage({ id: req.id, ok: true, durationMs: performance.now() - start, document });
  } catch (error) {
    ctx.postMessage({ id: req.id, ok: false, code: 'PARSE_FAILED', message: String(error) });
  }
};
