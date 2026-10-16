/**
 * pdf-view worker(classic worker)
 * pdfium 模块装配:pdfium.std.js(emscripten)+ pdfium-view-rs(no-modules 胶水)
 * 两者的初始化顺序与 wasmBinary 预取由本文件负责(见 public/pdfium/SOURCE.md)。
 */
importScripts('/pdfium/pdfium.std.js', '/glue/pdfium_view_rs.js');

let ready = null;

function ensure() {
  if (!ready) {
    ready = (async () => {
      // 该构建要求预取 wasm 并以 wasmBinary 传入
      const wasmBinary = await (await fetch('/pdfium/pdfium.std.wasm')).arrayBuffer();
      const pdfiumModule = await PDFiumModule({ wasmBinary });
      const rustModule = await wasm_bindgen('/glue/pdfium_view_rs_bg.wasm');
      if (!wasm_bindgen.initialize_pdfium_render(pdfiumModule, rustModule, false)) {
        throw new Error('pdfium-render 绑定失败');
      }
    })();
  }
  return ready;
}

self.onmessage = async ev => {
  const msg = ev.data;
  try {
    await ensure();
    if (msg.type === 'open') {
      const pages = wasm_bindgen.pageInfo(new Uint8Array(msg.bytes));
      self.postMessage({ type: 'meta', id: msg.id, pages });
    } else if (msg.type === 'render') {
      const t0 = performance.now();
      const image = wasm_bindgen.renderPage(msg.page, msg.width);
      self.postMessage({
        type: 'page',
        id: msg.id,
        page: msg.page,
        image,
        ms: performance.now() - t0,
      });
    }
  } catch (e) {
    self.postMessage({ type: 'error', id: msg.id, message: e && e.message ? e.message : String(e) });
  }
};
