# Office Viewer SDK 修订设计

## 结论

本方案把现有四个独立预览应用改造成可被第三方浏览器应用直接引用的 SDK。集成方式使用原生 DOM、Shadow DOM 和 Web Worker，不使用 iframe。

当前仓库中的 `apps/*` 保留为演示应用和回归入口；SDK 代码放在 `packages/` 下。monorepo 目录迁移方案继续作为仓库组织基线，本设计只补充第三方集成边界。

## 目标

- 支持 Word、Excel、PowerPoint、PDF 的浏览器端预览。
- 第三方应用只需要提供一个容器和文件数据。
- 解析和渲染继续在浏览器内完成，文件内容不发送到服务端。
- Worker、WASM、PDFium 资源可以从 npm 包、应用静态目录或 CDN 加载。
- 支持重复打开、取消、清空、销毁和多个 viewer 实例并行运行。
- 保持现有四个应用的视觉回归样例和解析能力。

## 非目标

- 不引入 iframe。
- 不在第一阶段提供编辑、批注、协同或导出能力。
- 不把四套 Rust crate 合并成一个 Cargo workspace。
- 不把解析器内部文档模型作为长期公共 API。
- 不在第一阶段同时维护 React、Vue、Angular 多套封装；先提供框架无关的 imperative API。

## 公共 API

```ts
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
  /** npm 包内置资源的基础地址；未指定时使用构建产物的相对地址。 */
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

export type OfficeViewerEvent =
  | { type: 'statechange'; state: 'idle' | 'loading' | 'ready' | 'error' | 'destroyed' }
  | { type: 'warning'; code: string; message: string }
  | { type: 'error'; error: OfficeViewerError };

export type OfficeViewerListener = (event: OfficeViewerEvent) => void;

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

export interface OfficeOpenResult {
  format: OfficeFormat;
  pageCount?: number;
  itemCount?: number;
  warnings: Array<{ code: string; message: string }>;
  durationMs: number;
  /** 各格式附加计数，供 Demo 使用；字段按格式变化。 */
  stats: Record<string, number>;
}

export interface OfficeViewerOptions {
  /** 未指定时根据 name 或文件签名推断；不支持推断时返回明确错误。 */
  format?: OfficeFormat;
  assets?: OfficeAssetOptions;
  /** 用于宿主应用自定义 Worker 创建策略或 CSP 场景。 */
  workerFactory?: OfficeWorkerFactory;
}

export interface OfficeViewer {
  open(request: OfficeOpenRequest): Promise<OfficeOpenResult>;
  clear(): void;
  resize(): void;
  destroy(): void;
  on(listener: OfficeViewerListener): () => void;
}

export function createOfficeViewer(
  container: HTMLElement,
  options?: OfficeViewerOptions,
): OfficeViewer;
```

典型调用保持单一入口：

```ts
const viewer = createOfficeViewer(container, { assets: { baseUrl: '/static/office/' } });
await viewer.open({ bytes, name: 'report.pptx', format: 'pptx', signal });
```

`OfficeOpenResult` 只返回稳定的格式、统计、警告和耗时信息。`WordDoc`、`PptDoc`、`ExcelDoc` 等解析器内部类型只在 adapter 内部使用，避免把 Rust/WASM 实现细节锁定到第三方代码。`signal` 与文件数据放在同一个 request 对象中，避免第三方调用方维护两套打开约定。

`open()` 的输入支持 `Blob`、`ArrayBuffer` 和 `Uint8Array`。调用方可以通过 `format` 明确指定格式；`name` 只用于展示和补充推断，不能作为唯一格式来源。传入可转移的 `ArrayBuffer` 后，SDK 不应悄悄改变调用方的所有权规则；默认复制或在文档中明确 transfer 行为。

## 包和目录边界

```text
packages/
  viewer-core/       # 公共类型、状态机、生命周期、错误和 Worker 通信
  viewer-word/       # Word adapter、Worker、WASM、DOM renderer
  viewer-excel/      # Excel adapter、Worker、WASM、DOM renderer
  viewer-ppt/        # PPT adapter、Worker、WASM、DOM renderer
  viewer-pdf/        # PDF adapter、classic Worker、PDFium 和 DOM renderer
  viewer/            # createOfficeViewer facade 和按格式动态加载
apps/
  word-view/         # 演示应用和视觉回归入口
  excel-view/
  ppt-view/
  pdf-view/
```

`viewer` facade 按实际格式动态加载 adapter，避免只使用 Word 时初始化 PDFium。必要时提供 `@office-view-suite/viewer/word` 等 subpath exports，便于不需要自动格式识别的宿主应用进一步缩小包体。

现有 `renderWordInto`、`renderExcelInto`、`renderPptInto` 先作为 adapter 内部实现重用。PDF 需要补出与其他格式一致的 renderer 入口，不能继续由 `apps/pdf-view/src/main.ts` 独占解析、懒渲染和页面 DOM 管理。

## Worker 和 WASM 资产

### Word、Excel、PPT

- 每个 adapter 生成独立的 ESM Worker 入口和 WASM 资源。
- Worker 和 WASM 通过构建产物中的相对 URL 或 `import.meta.url` 解析。
- 不允许依赖调用方站点根路径。
- 包的 `exports`、`files` 和类型声明必须包含运行时需要的 Worker、JS glue 和 `.wasm` 文件。

### PDF

PDFium 当前使用 classic Worker 和 `importScripts`，因此单独定义资产解析器：

```ts
export interface PdfAssetOptions {
  workerUrl: string;
  pdfiumJsUrl: string;
  pdfiumWasmUrl: string;
  glueJsUrl: string;
  glueWasmUrl: string;
}
```

默认值由 SDK 构建产物提供；宿主应用可以通过 `assets.baseUrl` 或显式 URL 覆盖。Worker 内部禁止使用 `/worker/pdf-worker.js`、`/pdfium/...`、`/glue/...` 这类绝对根路径。CDN 使用时必须验证静态资源的 CORS、MIME 类型和 CSP 要求。

## 生命周期和状态

viewer 使用以下状态：`idle`、`loading`、`ready`、`error`、`destroyed`。

- `open()` 开始新会话时取消上一会话的解析、渲染和待处理 Promise。
- 每个会话都有独立 request id，旧响应不能写入新文档。
- `AbortSignal` 触发后返回 `ABORTED`，不把取消当成解析失败。
- `clear()` 清理当前 DOM 和观察器，但保留可复用的 Worker。
- `destroy()` 终止 Worker，断开 `IntersectionObserver`、`ResizeObserver`，取消 `requestAnimationFrame`/idle 任务，移除事件监听，并使后续调用返回 `DESTROYED`。
- `dispose()` 作为兼容别名可以保留，但公共文档统一使用 `destroy()`。

## DOM 和布局规则

- 所有格式统一由 SDK 在宿主容器内创建 Shadow DOM，避免 PDF 与其他格式的样式隔离行为不一致。SDK 只接管自己创建的 shadow root，不重置宿主容器的 `class`、`style` 或子节点。
- SDK 不接管宿主页面的文件选择、拖拽、状态条和样例按钮；这些属于演示应用或宿主产品 UI。
- Word、PPT、Excel、PDF 都通过 `ResizeObserver` 响应容器尺寸变化。
- 公共 CSS 只规定内部布局和必要的 `:host` 默认行为；主题通过 CSS 自定义属性或显式配置覆盖。
- 宿主容器被移除后，SDK 至少通过 `destroy()` 明确释放；必要时可在开发模式检测 detached host 并给出警告。

## 错误和隐私

错误必须包含稳定 `code` 和面向开发者的 `message`，至少包括：`UNSUPPORTED_FORMAT`、`INVALID_SOURCE`、`ASSET_LOAD_FAILED`、`PARSE_FAILED`、`RENDER_FAILED`、`ABORTED`、`DESTROYED`。

SDK 只从配置的本地或静态资产地址加载解析引擎，不上传文档内容。文档需要明确：宿主若把 `assets` 指向第三方 CDN，该 CDN 将接收资源请求信息，但不会接收文档 bytes。

## 发布和浏览器边界

- 第一阶段发布 ESM 包，支持 Vite、Webpack、Rollup 和原生 ESM；不承诺 UMD 和 SSR。
- 实施验证已覆盖 Vite 和原生 ESM；Webpack/Rollup 的兼容性仍需在对应消费工程实测，暂不标记为已验证。
- 包发布内容只包含 `dist`、类型声明、Worker、WASM 和必要的许可证文件，不包含 Rust vendor、测试样例和构建缓存。
- README 必须写明浏览器需要 Web Worker、WebAssembly、Shadow DOM 和 `ResizeObserver`；不满足时返回可识别错误或提供 polyfill 说明。
- 公开包采用 semver；解析模型字段不属于兼容性承诺，公共 API 和错误码属于兼容性承诺。

## 验收标准

1. 独立的 vanilla/Vite 消费项目能安装包并打开四种格式，不依赖仓库源码路径。
2. Word、Excel、PPT、PDF 的 Worker 和 WASM 均从包内相对路径或配置的 `assets` 地址加载。
3. 部署到 `/office-view/` 子路径和 CDN 地址时，PDF 不再请求站点根路径资源。
4. 两个 viewer 实例可以同时运行；连续打开、取消、清空和销毁不会出现旧响应回写或未处理 Promise。
5. 调整宿主容器大小后，四种格式均重新适配布局。
6. 现有四个 demo 的 `typecheck`、`build` 和样例视觉回归通过。
7. 发布包不包含 Rust vendor、`target`、`node_modules`、demo 样例和开发缓存。
