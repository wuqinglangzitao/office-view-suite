/// <reference lib="webworker" />
import type { ExcelDoc } from '../core/types';

export interface ExcelRequest {
  id: number;
  name: string;
  bytes: ArrayBuffer;
}

export type ExcelResponse =
  | { id: number; ok: true; durationMs: number; document: ExcelDoc }
  | { id: number; ok: false; message: string };

type WasmModule = typeof import('../wasm/excel_view_rs');

let modulePromise: Promise<WasmModule> | null = null;

function load(): Promise<WasmModule> {
  if (!modulePromise) {
    modulePromise = import('../wasm/excel_view_rs').then(async m => {
      await m.default();
      return m;
    });
  }
  return modulePromise;
}

const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = async (ev: MessageEvent<ExcelRequest>) => {
  const req = ev.data;
  try {
    if (!/\.(xlsx|xlsm)$/i.test(req.name)) {
      reply({
        id: req.id,
        ok: false,
        message: 'excel-view 仅支持 .xlsx/.xlsm(97-2003 二进制请走 anydoc 兜底层)',
      });
      return;
    }
    const m = await load();
    const t0 = performance.now();
    const document = m.toDocument(new Uint8Array(req.bytes)) as unknown as ExcelDoc;
    reply({ id: req.id, ok: true, durationMs: performance.now() - t0, document });
  } catch (e) {
    reply({ id: req.id, ok: false, message: e instanceof Error ? e.message : String(e) });
  }
};

function reply(msg: ExcelResponse): void {
  ctx.postMessage(msg);
}
