/* PDFium classic worker. All runtime assets are supplied by the adapter. */
let ready;
let loadedAssets;
let activeSession;
let queue = Promise.resolve();

async function ensure(assets) {
  if (!assets) throw new Error('PDF 资源地址未配置');
  if (ready) {
    if (JSON.stringify(assets) !== JSON.stringify(loadedAssets)) throw new Error('PDF Worker 资源配置不可在初始化后修改');
    return ready;
  }
  loadedAssets = assets;
  ready = (async () => {
    const url = value => new URL(value, self.location.href).href;
    importScripts(url(assets.pdfiumJsUrl), url(assets.glueJsUrl));
    const response = await fetch(url(assets.pdfiumWasmUrl));
    if (!response.ok) throw new Error(`PDFium WASM 请求失败 (${response.status})`);
    const wasmBinary = await response.arrayBuffer();
    const pdfiumModule = await PDFiumModule({ wasmBinary });
    const rustModule = await wasm_bindgen(url(assets.glueWasmUrl));
    if (!wasm_bindgen.initialize_pdfium_render(pdfiumModule, rustModule, false)) throw new Error('pdfium-render 绑定失败');
  })();
  return ready;
}

// Keep asynchronous initialization and each synchronous document/render
// operation in one ordered queue; a render never uses another open's PDF.
self.onmessage = event => {
  const message = event.data;
  queue = queue.then(() => handle(message));
};

async function handle(message) {
  let code = 'ASSET_LOAD_FAILED';
  try {
    await ensure(message.assets);
    if (message.type === 'open') {
      code = 'PARSE_FAILED';
      activeSession = undefined;
      const pages = wasm_bindgen.pageInfo(new Uint8Array(message.bytes));
      activeSession = message.session;
      self.postMessage({ type: 'meta', id: message.id, session: message.session, pages });
    } else if (message.type === 'render') {
      code = 'RENDER_FAILED';
      if (activeSession !== message.session) throw new Error('PDF 文档会话已失效');
      if (!Number.isInteger(message.page) || message.page < 0 || !Number.isInteger(message.width) || message.width < 1) {
        throw new Error('PDF 页面或尺寸无效');
      }
      const image = wasm_bindgen.renderPage(message.page, message.width);
      self.postMessage({ type: 'page', id: message.id, session: message.session, page: message.page, image });
    } else {
      code = 'PARSE_FAILED';
      throw new Error('未知 PDF Worker 操作');
    }
  } catch (error) {
    self.postMessage({ type: 'error', id: message.id, session: message.session, code, message: error instanceof Error ? error.message : String(error) });
  }
}
