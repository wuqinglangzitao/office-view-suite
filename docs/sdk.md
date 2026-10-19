# 文件预览 SDK 接入

SDK 使用原生 DOM、Shadow DOM、Worker 和 WASM，在浏览器内解析文件，不使用 iframe，也不上传文件内容。`apps/*` 是使用同一 SDK 的演示应用。

## 构建与安装

当前仓库已提供可打包的 ESM 包，尚未发布到 npm。首次从源码构建：

```sh
pnpm install
pnpm wasm       # 首次或 Rust 解析器修改后执行，需要 Rust / wasm-pack
pnpm build
pnpm typecheck
pnpm test
pnpm test:packages
```

`pnpm test:packages` 会将六个包打成 `.tgz`，在系统临时目录建立独立项目，仅从这些安装包安装运行时依赖，再执行类型检查和生产构建。临时项目路径记录在 `output/packed-consumer.json`。在包尚未发布时，跨项目安装需要提供全部六个安装包；该脚本给出了完整的本地依赖和 `pnpm.overrides` 配置。

Rust 源码和各自的 Cargo.lock 位于 `packages/viewer-{word,excel,ppt,pdf}/rs/`，生成的 WASM/glue 位于同包的 `src/wasm/`。`pnpm wasm` 使用 `--locked` 构建并记录源码、vendor、仓库 Cargo 配置、工具链声明、构建脚本及输出文件的指纹。SDK 构建会拒绝缺失或过期的产物，并提示 `pnpm wasm:<格式>`；TypeScript 修改无需重建 WASM。指纹用于防止本地源码与生成产物错配，不是跨平台逐字节可复现构建的证明。PDFium 本体是 `third_party/pdfium/` 中的上游预编译产物，`wasm:pdf` 只编译本项目 Rust glue。

`pnpm test` 包含真实 Word/Excel/PPT WASM 输出与当前 TypeScript 模型的契约验证；`pnpm test:rust` 运行这三个 crate 的原生单元测试。PDF glue 的运行验证在浏览器回归中完成。

| 包 | 职责 |
| --- | --- |
| `@office-view-suite/viewer` | 推荐的公共入口，按格式动态加载 |
| `@office-view-suite/viewer-core` | 公共类型、错误、格式识别和 Worker 通信 |
| `@office-view-suite/viewer-word` | Word 解析与渲染 |
| `@office-view-suite/viewer-excel` | Excel 解析与渲染 |
| `@office-view-suite/viewer-ppt` | PowerPoint 解析与渲染 |
| `@office-view-suite/viewer-pdf` | PDFium 解析与懒渲染 |

适配器的文档模型和底层接口属于内部实现；第三方应用使用 `viewer` 包的 API。未提供 `viewer/word` 等子路径入口。

## 最小用法

```ts
import { createOfficeViewer } from '@office-view-suite/viewer';

const container = document.querySelector<HTMLElement>('#preview')!;
const viewer = createOfficeViewer(container);

const unsubscribe = viewer.on(event => {
  if (event.type === 'error') console.error(event.error.code, event.error.message);
});

// File 也是 Blob；ArrayBuffer 和 Uint8Array 同样可用。
const result = await viewer.open({ bytes: file, name: file.name });
console.log(result.format, result.pageCount, result.itemCount, result.warnings);

viewer.clear();       // 清空预览，可再次 open
viewer.resize();      // 隐藏容器重新显示时可主动调用
unsubscribe();
viewer.destroy();     // 组件卸载时调用，幂等
```

React/Vue 等框架在挂载后创建实例，在卸载时调用 `destroy()` 即可。容器应具有可测量的宽度；SDK 创建自己的子元素及 Shadow Root，不重置容器的 class、style、原有子节点或 Shadow Root。Word 保持原页面尺寸，Excel 保留表格和选中的工作表，PPT/PDF 随容器宽度适配。

### 输入和结果

`open({ bytes, name?, format?, signal? })` 支持 `.doc/.docx/.docm`、`.xlsx/.xlsm`、`.pptx`、`.pdf`。先检查文件内容，再选择解析器；例如 XLSX 误命名为 `.pdf` 仍按 Excel 打开。显式 `format` 与识别到的格式家族冲突时返回 `INVALID_SOURCE`。同一 OOXML 家族的 `.docx/.docm`、`.xlsx/.xlsm` 可由显式格式或文件名细化标记，因为宏格式不一定包含 VBA 条目。

预检只检查 PDF 标识、Office ZIP/ZIP64 目录边界与主条目、Word 复合文档标识，不解压或验证完整文档语义。明确损坏的容器或与声明格式不符的内容返回 `INVALID_SOURCE`，未识别且没有受支持格式提示的内容返回 `UNSUPPORTED_FORMAT`；深层损坏仍由解析器返回 `PARSE_FAILED`。不支持 `.xls/.ppt`，指定 `format` 不能绕过预检。

公共入口取得输入副本并预检一次，再将内部持有的 buffer 交给对应 Worker；不会 detach 调用者的 ArrayBuffer 或 Uint8Array。PDF 仅保留视口及约 300px 预加载区域内的页面位图，离屏保留布局占位，返回时重绘。没有 IntersectionObserver 时仍使用全量绘制降级路径；这项优化不限制 PDF 引擎自身的文档内存。

SDK 复制文件数据，传给 Worker 后不会分离调用方的 ArrayBuffer。结果包含 `format`、`durationMs`、`warnings: Array<{code,message}>` 和格式相关的 `stats`；PPT/PDF 提供 `pageCount`，Excel 提供 `itemCount`。Word 分页渐进完成，当前不提供最终页数。`open()` 表示文档已解析并挂载；Word 后续分页和 PDF 可视页渲染仍可能继续。

```ts
const abort = new AbortController();
const opening = viewer.open({ bytes, format: 'pptx', signal: abort.signal });
abort.abort();
try { await opening; } catch (error) { /* error.code === 'ABORTED' */ }
```

同一实例再次 `open()` 会取消前一个会话。`clear()` 取消当前工作并释放渲染 DOM；空闲 Worker 可复用，忙碌 Worker 必要时终止。`destroy()` 终止所有 Worker、取消渲染任务、移除监听和观察器；之后 `open()` 返回 `DESTROYED`。多个实例相互独立。

### 事件与错误

`on(listener)` 返回取消订阅函数，事件包括 `statechange`、`warning`、`error`。状态依次可能为 `idle/loading/ready/error/destroyed`。后台 PDF 页面和 Excel 切表、滚动渲染错误通过 `error` 事件报告；接入方应同时处理 `open()` 的 Promise 拒绝和此事件。

| code | 含义 |
| --- | --- |
| `INVALID_SOURCE` | 数据类型不支持、空文件、读取失败、明确损坏的容器或格式冲突 |
| `UNSUPPORTED_FORMAT` | 格式不支持或无法识别 |
| `ASSET_LOAD_FAILED` | Worker、WASM、PDFium 或适配器加载失败 |
| `PARSE_FAILED` | 文档解析失败 |
| `RENDER_FAILED` | DOM/Canvas 渲染失败 |
| `ABORTED` | 请求取消、被替换或预览被清空 |
| `DESTROYED` | 实例已经销毁 |

### 只读预览控制

```ts
viewer.setZoom(1.5);             // 150%，支持 0.1–4 的有限数值
viewer.setZoom('fit-width');     // 适应容器宽度，随 resize 调整
console.log(viewer.getZoom());   // { mode: 'fixed' | 'fit-width', scale: number }
console.log(viewer.getCapabilities()); // { zoom: true, fitWidth: true }
```

默认比例为 1。缩放仅作用于 SDK 自己创建的内容，不修改宿主容器样式；非法比例返回 `INVALID_SOURCE`，销毁后设置比例返回 `DESTROYED`。这些接口只控制预览，不提供搜索、单元格/范围定位、选区编辑、打印或导出。

`fit-width` 同样受 0.1–4 的比例范围约束；超宽内容缩到 10% 后仍可能需要横向滚动。SDK 内容区负责缩放后的横向溢出，宿主页面负责外层纵向滚动；Excel 表格保留自己的滚动区域。PDF 当前通过 CSS 放大已有 Canvas，较大比例可能降低文字清晰度。

Excel 只挂载当前工作表，往返切换保留各表滚动位置。大表使用可见区域渲染；当前视口以外的内容仍可通过滚动查看，但不常驻 DOM，因此宿主不应通过遍历 DOM 提取整份文件数据。

超过 10,000 个潜在单元格的工作表使用虚拟视口。隐式行高和列宽由文本测量估算；字体加载完成后重新测量。极小行高、列宽或过多绘图会使单个视口过密，超过 20,000 个单元格及绘图节点时返回 `RENDER_FAILED`，避免无界挂载。这是异常密度保护；基准中 6,000 个单元格的门槛只对应固定测试环境和样例。

## Worker 和 WASM 部署

默认由相对于包的 URL 加载资源。Vite 构建会识别并输出这些资源，支持应用部署在 `/office-view/` 等子路径下，不要求站点根目录存在 `/pdfium` 或 `/glue`。

需要固定静态资源目录时，将四个适配器包的 `dist/assets/` 内容合并复制到应用公开目录：

```text
/static/office/
  word/worker.js, word_view_rs_bg.wasm
  excel/worker.js, excel_view_rs_bg.wasm
  ppt/worker.js, ppt_view_rs_bg.wasm
  pdf/worker.js
  pdf/pdfium/pdfium.std.js, pdfium.std.wasm
  pdf/glue/pdfium_view_rs.js, pdfium_view_rs_bg.wasm
```

每个适配器构建还会生成 `dist/asset-manifest.json`，包含相对于 dist 的资源路径、大小、SHA-256 和建议 MIME。`pnpm test:packages` 校验实际安装包中的清单并按清单复制部署资源；清单不会改变浏览器默认资源加载。文件缺失、额外未登记文件或哈希不符都会使校验失败。HTTP MIME/CORS 仍由宿主服务器配置，并由浏览器正负场景测试检查。

```ts
const viewer = createOfficeViewer(container, {
  assets: { baseUrl: '/static/office/' },
});
```

还可以覆盖 `assets.word/excel/ppt.{workerUrl,wasmUrl}`，以及 `assets.pdf.{workerUrl,pdfiumJsUrl,pdfiumWasmUrl,glueJsUrl,glueWasmUrl}`。所有显式 URL 均可使用相对于宿主页的地址。

浏览器要求 Worker 入口同源。使用 CDN 时，推荐将 Worker 入口留在应用同源目录，仅覆盖 WASM/PDFium 资源 URL；另一种方式是提供 `workerFactory` 创建同源 Blob 引导程序：

```ts
const workerBlobs: string[] = [];
const viewer = createOfficeViewer(container, {
  assets: { baseUrl: 'https://cdn.example.com/office/' },
  workerFactory(url, options) {
    const source = options?.type === 'module'
      ? `import ${JSON.stringify(url.href)}`
      : `importScripts(${JSON.stringify(url.href)})`;
    const blobUrl = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    workerBlobs.push(blobUrl);
    return new Worker(blobUrl, options);
  },
});
// 宿主组件卸载时：
viewer.destroy();
workerBlobs.forEach(url => URL.revokeObjectURL(url));
```

资源服务器应返回正确的 JS/WASM MIME 类型并配置 CORS。CSP 需允许所选 Worker 来源、脚本/资源来源和 WebAssembly；采用 Blob 引导时还需允许 `worker-src blob:`。SDK 内部使用样式元素、内联样式和文档 data 图片，宿主应按自身策略允许这些能力。静态 CDN 只收到引擎资源请求，SDK 不向 CDN 发送文档 bytes。

## 原生 ESM

将六个包各自的 `dist` 目录部署到独立 URL，并用 import map 映射六个包名到相应的 `index.js`。保留每个适配器的 `assets` 相对目录。例如：

```html
<script type="importmap">
{"imports": {
  "@office-view-suite/viewer": "/sdk/viewer/index.js",
  "@office-view-suite/viewer-core": "/sdk/viewer-core/index.js",
  "@office-view-suite/viewer-word": "/sdk/viewer-word/index.js",
  "@office-view-suite/viewer-excel": "/sdk/viewer-excel/index.js",
  "@office-view-suite/viewer-ppt": "/sdk/viewer-ppt/index.js",
  "@office-view-suite/viewer-pdf": "/sdk/viewer-pdf/index.js"
}}
</script>
<script type="module">
  import { createOfficeViewer } from '@office-view-suite/viewer';
  const viewer = createOfficeViewer(document.querySelector('#preview'));
</script>
```

## 验证和边界

已验证 Chrome 中的 Vite 生产产物、原生 ESM、四种格式、子路径、跨域资源、自定义 Worker、双实例、连续打开、取消和销毁。需要现代浏览器的 Worker、WebAssembly、Shadow DOM、ResizeObserver；PDF 使用 Canvas，默认使用 IntersectionObserver 懒渲染。SSR 和 UMD 不在当前范围。Webpack/Rollup、Safari/Firefox 尚未完成实测，不作已验证承诺。

复用现有 Rust 解析器和 DOM 渲染器，因此不承诺和 Microsoft Office 像素级一致；字体是否安装、Office 特性覆盖和原解析器限制仍然适用。

浏览器回归复用已安装的 Playwright，不增加 SDK 运行时依赖：先运行 `pnpm test:packages`，再运行 `PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs pnpm test:browser`。截图和结果位于 `output/playwright/`。本次只准备本地安装包，未发布 npm。

`pnpm test:visual` 对比 Git 基线 `ee6dbbe` 的旧渲染器和打包 SDK，覆盖 6 个样本共 45 页／工作表，包括架构评审 PPT 的全部 39 页；另执行 9 组 Word 缩放与渐进分页检查。可用 `SDK_BASELINE_REF` 指定另一个包含旧渲染器的提交；该比较使用同一解析结果验证迁移前后渲染一致性，不表示与 Office 原软件像素一致。

`PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs pnpm test:excel` 验证 Excel 懒挂载、滚动恢复、大表 DOM 上限、合并/冻结与缩放坐标。9 个合成 DTO 衡量渲染成本；3 个已签入 XLSX 和 1 个完整 OOXML 生成文件独立验证样式、合并、图片、富文本及公式缓存字段，加载/解析耗时另列。它从 `EXCEL_BASELINE_REF`（默认 `0ac7d51`）提取优化前 renderer，固定视口预热一次、测量五次，输出 `output/excel-preview-results.json`；为避免原版生成过量 DOM，超过安全阈值的旧版场景明确记为跳过，不声称对应旧版耗时。

## 第三方许可随包部署

每个适配器的安装包带有 `dist/THIRD_PARTY_NOTICES.md` 和 `dist/licenses/`。PDF 适配器还包含与 `pdfium-lib 8046d` 二进制对应的许可材料：

- `dist/licenses/pdfium/`：来源记录、构建证据、组件清单、哈希清单及许可证正文。
- `dist/assets/pdf/pdfium/`：JS/WASM 旁保留相同材料；按本文复制整个 `dist/assets/` 目录时，声明会一同部署。

PDFium 许可涵盖 PDFium 及内嵌组件、字体、Emscripten 运行库和启用的 ports。署名和完整原文见 [NOTICE.md](../third_party/pdfium/NOTICE.md)；构建追溯见 [BUILD_EVIDENCE.md](../third_party/pdfium/BUILD_EVIDENCE.md)。其中包含 IJG 和 FreeType 要求的产品文档署名，分发时应保留。

若使用 Vite 等工具仅打包被代码引用的资源，许可证文档不会因此自动进入最终网站。宿主应用需另外部署各适配器的 `dist/THIRD_PARTY_NOTICES.md` 和 `dist/licenses/`，并从“关于 / 开源许可”页提供访问；SDK 二次分发也应保留这些材料。无须在每个预览页面展示全文。

维护者运行 `pnpm verify:pdfium` 可离线校验源文件。PDF SDK 构建会校验实际输出的 JS/WASM 与许可文件，`pnpm test:packages` 还会核对 npm tarball 内的材料；文件缺失或哈希不符会失败。升级 PDFium 时先按构建证据文档重新追溯、收集许可，再更新 `provenance.json`，不能只改二进制或刷新哈希来跳过复核。

这些材料仅覆盖所记录的第三方构建；项目自身的发布许可证仍应由仓库所有者确定，示例文件等业务素材另行授权。
