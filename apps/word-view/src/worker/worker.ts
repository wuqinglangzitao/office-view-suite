/// <reference lib="webworker" />
import type { WordDoc } from '../core/types';

export interface WordRequest {
  id: number;
  name: string;
  bytes: ArrayBuffer;
}

export type WordResponse =
  | { id: number; ok: true; durationMs: number; document: WordDoc }
  | { id: number; ok: false; message: string };

type WasmModule = typeof import('../wasm/word_view_rs');

let modulePromise: Promise<WasmModule> | null = null;

function load(): Promise<WasmModule> {
  if (!modulePromise) {
    modulePromise = import('../wasm/word_view_rs').then(async m => {
      await m.default();
      return m;
    });
  }
  return modulePromise;
}

const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = async (ev: MessageEvent<WordRequest>) => {
  const req = ev.data;
  try {
    const ext = req.name.toLowerCase().split('.').pop() ?? '';
    if (!['doc', 'docx', 'docm'].includes(ext)) {
      reply({ id: req.id, ok: false, message: `word-view 仅支持 .doc/.docx,收到 .${ext}` });
      return;
    }
    const m = await load();
    const t0 = performance.now();
    const document = m.toDocument(new Uint8Array(req.bytes)) as unknown as WordDoc;
    reply({ id: req.id, ok: true, durationMs: performance.now() - t0, document });
  } catch (e) {
    reply({ id: req.id, ok: false, message: e instanceof Error ? e.message : String(e) });
  }
};

function reply(msg: WordResponse): void {
  ctx.postMessage(msg);
}
