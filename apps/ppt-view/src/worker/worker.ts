/// <reference lib="webworker" />
import type { PptDoc } from '../core/types';

export interface PptRequest {
  id: number;
  name: string;
  bytes: ArrayBuffer;
}

export type PptResponse =
  | { id: number; ok: true; durationMs: number; document: PptDoc }
  | { id: number; ok: false; message: string };

type WasmModule = typeof import('../wasm/ppt_view_rs');

let modulePromise: Promise<WasmModule> | null = null;

function load(): Promise<WasmModule> {
  if (!modulePromise) {
    modulePromise = import('../wasm/ppt_view_rs').then(async m => {
      await m.default();
      return m;
    });
  }
  return modulePromise;
}

const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = async (ev: MessageEvent<PptRequest>) => {
  const req = ev.data;
  try {
    if (!/\.pptx$/i.test(req.name)) {
      reply({
        id: req.id,
        ok: false,
        message: 'ppt-view 仅支持 .pptx(97-2003 二进制 .ppt 请走 anydoc 兜底层)',
      });
      return;
    }
    const m = await load();
    const t0 = performance.now();
    const document = m.toDocument(new Uint8Array(req.bytes)) as unknown as PptDoc;
    reply({ id: req.id, ok: true, durationMs: performance.now() - t0, document });
  } catch (e) {
    reply({ id: req.id, ok: false, message: e instanceof Error ? e.message : String(e) });
  }
};

function reply(msg: PptResponse): void {
  ctx.postMessage(msg);
}
