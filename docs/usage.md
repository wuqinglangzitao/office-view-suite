# Office View Suite 使用文档

适用版本：源码 `@office-view-suite/viewer@0.2.0`（尚未发布）；私服已发布 `0.1.2` 可使用原有接口，水印需本次源码打包产物。更新日期：2026-10-22。

本文面向业务前端开发人员，介绍如何将文档预览嵌入页面、上传组件和附件弹窗。SDK 使用原生 DOM、Shadow DOM、Worker 和 WASM 在浏览器内解析文件，PDF 使用 PDFium 渲染，无需 iframe；SDK 不上传文档内容。

源码与构建维护说明见 [SDK 接入与部署](sdk.md)，Vue 2 配套组件见 [Vue 2 + Vite](vue2.md)。

## 支持范围

| 类型 | 支持的扩展名 | 预览行为 |
| --- | --- | --- |
| Word | `.doc`、`.docx`、`.docm` | 保留文档页面，渐进分页；支持正文、表格、页眉页脚等内容 |
| Excel | `.xlsx`、`.xlsm` | 工作表切换、合并单元格、样式及绘图；大表按可见区域渲染 |
| PowerPoint | `.pptx` | 按幻灯片展示文字、形状、图片和表格，随容器宽度适配 |
| PDF | `.pdf` | PDFium + Canvas，可见页面懒渲染，离屏回收位图 |

`.docm`、`.xlsm` 只预览内容，不执行 VBA 宏。旧版 `.xls`、`.ppt` 不在支持范围。文件内容参与格式识别，修改扩展名不能完成格式转换。

SDK 提供解析、渲染、缩放、事件和资源释放接口。文件选择、弹窗、加载提示、缩放按钮、百分比显示、还原及原文件下载由业务应用组织；今天的附件弹窗已通过这些接口完成接入。

## 安装

在业务项目的 `.npmrc` 中添加公司私服作用域：

```ini
@office-view-suite:registry=http://npm.dev.e6gpshk.com:38085/repository/npm-group/
```

安装公共入口：

```sh
pnpm add @office-view-suite/viewer@0.1.2
```

使用 npm 的项目可执行 `npm install @office-view-suite/viewer@0.1.2`。其余五个 SDK 包会作为依赖自动安装。私服访问与登录沿用公司 npm 配置，凭据保存在用户级配置中。

消费已发布的安装包不需要 Rust、`wasm-pack` 或本地 `vendor/office-view-suite`。离线安装需要同时提供六个 SDK 安装包，具体见 [构建与安装](sdk.md#构建与安装)。

## Vite 配置

将以下内容合并到业务项目的 `vite.config.ts`，保留已有 Vue 等框架插件：

```ts
import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    exclude: [
      '@office-view-suite/viewer',
      '@office-view-suite/viewer-core',
      '@office-view-suite/viewer-word',
      '@office-view-suite/viewer-excel',
      '@office-view-suite/viewer-ppt',
      '@office-view-suite/viewer-pdf',
    ],
  },
  build: {
    target: 'es2022',
    assetsInlineLimit(filePath) {
      if (filePath.includes('/node_modules/@office-view-suite/')) return false;
      return undefined;
    },
  },
});
```

`optimizeDeps.exclude` 让开发服务器保留包内 Worker/WASM 的相对路径；`assetsInlineLimit` 让 SDK 资源在生产构建中保持独立文件，避免 PDF classic Worker 被内联为失去同源上下文的 data URL，其余应用资源沿用 Vite 默认阈值。修改后重启开发服务器，必要时执行 `vite --force` 清除旧预打包缓存。生产构建由 Vite 输出引擎资源，无需额外加载全局脚本。

## 页面接入示例

### 容器与工具栏

在页面中放置以下结构：

```html
<section class="office-preview-demo">
  <div class="preview-toolbar">
    <input id="office-file" type="file" accept=".doc,.docx,.docm,.xlsx,.xlsm,.pptx,.pdf" />
    <button id="zoom-out" type="button">缩小</button>
    <output id="zoom-label">100%</output>
    <button id="zoom-in" type="button">放大</button>
    <button id="zoom-reset" type="button">还原</button>
    <span id="preview-status" role="status"></span>
  </div>
  <div id="office-preview"></div>
</section>
```

为外层分配高度，预览区占据工具栏之外的剩余空间：

```css
.office-preview-demo {
  display: flex;
  flex-direction: column;
  height: 75vh;
  min-height: 0;
}

.preview-toolbar { flex: none; }

#office-preview {
  flex: 1;
  width: 100%;
  min-width: 0;
  min-height: 0;
  overflow: auto;
}
```

### 创建实例与加载文件

在上述 DOM 挂载后执行以下 TypeScript。示例包含文件选择、缩放百分比、还原、错误去重和卸载清理：

```ts
import {
  createOfficeViewer,
  type OfficeViewerError,
} from '@office-view-suite/viewer';

const input = document.querySelector<HTMLInputElement>('#office-file')!;
const container = document.querySelector<HTMLElement>('#office-preview')!;
const status = document.querySelector<HTMLElement>('#preview-status')!;
const label = document.querySelector<HTMLOutputElement>('#zoom-label')!;
const minus = document.querySelector<HTMLButtonElement>('#zoom-out')!;
const plus = document.querySelector<HTMLButtonElement>('#zoom-in')!;
const reset = document.querySelector<HTMLButtonElement>('#zoom-reset')!;
const viewer = createOfficeViewer(container);
const reported = new WeakSet<OfficeViewerError>();

function reportError(error: OfficeViewerError) {
  if (['ABORTED', 'DESTROYED'].includes(error.code) || reported.has(error)) return;
  reported.add(error);
  status.textContent = error.message;
  console.error(error.code, error);
}

function updateZoomLabel() {
  label.textContent = `${Math.round(viewer.getZoom().scale * 100)}%`;
}

function changeZoom(step: number) {
  const next = Number((viewer.getZoom().scale + step).toFixed(2));
  viewer.setZoom(Math.min(4, Math.max(0.1, next)));
  updateZoomLabel();
}

const onMinus = () => changeZoom(-0.1);
const onPlus = () => changeZoom(0.1);
const onReset = () => {
  viewer.setZoom(1);
  updateZoomLabel();
};
const onFileChange = () => {
  const file = input.files?.[0];
  if (!file) return;
  onReset();
  void viewer.open({ bytes: file, name: file.name }).catch(reportError);
};

const unsubscribe = viewer.on(event => {
  if (event.type === 'error') reportError(event.error);
  if (event.type === 'warning') console.warn(event.code, event.message);
  if (event.type === 'statechange') {
    if (event.state === 'loading') status.textContent = '加载中…';
    if (event.state === 'ready') {
      status.textContent = '';
      updateZoomLabel();
    }
  }
});

input.addEventListener('change', onFileChange);
minus.addEventListener('click', onMinus);
plus.addEventListener('click', onPlus);
reset.addEventListener('click', onReset);

// 在页面或组件卸载回调中调用。
export function disposePreview() {
  input.removeEventListener('change', onFileChange);
  minus.removeEventListener('click', onMinus);
  plus.removeEventListener('click', onPlus);
  reset.removeEventListener('click', onReset);
  unsubscribe();
  viewer.destroy();
}
```

`open()` 的 Promise 和 `error` 事件可能报告同一个错误，示例通过对象身份去重。切换文件产生的 `ABORTED` 和组件卸载产生的 `DESTROYED` 通常无需弹出错误提示。

## 加载远程附件

`open()` 接收文件数据。远程 URL 的鉴权、请求头、超时和下载进度由业务项目处理；如果项目使用 Axios 等请求层，应沿用已有登录和下载逻辑，取得 `Blob` 或 `ArrayBuffer` 后传入 SDK。

下面是使用 `fetch` 的业务侧辅助函数。它在切换附件时同时中止旧下载和旧解析，避免先发起的慢请求覆盖后选择的文件：

```ts
import { type OfficeViewer } from '@office-view-suite/viewer';

export function createAttachmentLoader(viewer: OfficeViewer) {
  let active: AbortController | undefined;

  function clear() {
    active?.abort();
    active = undefined;
    viewer.clear();
  }

  return {
    async open(url: string, name: string) {
      clear();
      const controller = new AbortController();
      active = controller;
      try {
        const response = await fetch(url, {
          signal: controller.signal,
          credentials: 'same-origin',
        });
        if (!response.ok) throw new Error(`附件下载失败：HTTP ${response.status}`);
        const bytes = await response.blob();
        if (controller.signal.aborted || active !== controller) return;
        await viewer.open({ bytes, name, signal: controller.signal });
      } catch (error) {
        if (!controller.signal.aborted && active === controller) throw error;
      } finally {
        if (active === controller) active = undefined;
      }
    },
    clear,
  };
}
```

业务调用 `loader.open(url, fileName)` 并处理其 Promise 拒绝；关闭弹窗时调用 `loader.clear()`，组件销毁时再调用 `viewer.destroy()`。错误处理需同时考虑业务下载错误和 SDK 错误。跨域附件需配置 CORS；跨站 Cookie 或 Token 由项目请求层按实际接口约定提供。

SDK 自动取消的是已经进入 `open()` 的旧会话。宿主发出的 `fetch` 仍需自己取消。SDK 复制传入数据，Worker 通信不会 detach 调用方持有的 `ArrayBuffer`。

## 公共 API

### 创建参数

`createOfficeViewer(container, options?)` 返回独立预览实例。容器应具有可测量的宽度，同一页面的多个预览应使用各自的容器和实例。

| 参数 | 用途 |
| --- | --- |
| `container: HTMLElement` | 挂载位置；由宿主页面决定宽度、高度和外层滚动 |
| `options.format` | 可选的格式提示，通常让 SDK 根据文件识别即可 |
| `options.assets` | 可选的 Worker、WASM、PDFium 资源 URL 配置 |
| `options.workerFactory` | 可选的自定义 Worker 创建函数，详见部署文档 |
| `options.watermark` | 可选文字水印，默认 `null`；`0.2.0` 新增 |

### 实例方法

| 方法 | 用途 |
| --- | --- |
| `open({ bytes, name?, format?, signal? })` | 解析并挂载文件，返回 `Promise<OfficeOpenResult>`；再次调用会替换旧会话 |
| `clear()` | 取消当前会话并清空预览，实例可再次打开文件 |
| `resize()` | 在弹窗打开、Tab 恢复或容器尺寸改变后重新测量 |
| `setZoom(0.1 ... 4)` | 设置固定比例，`1` 表示 100% |
| `setZoom('fit-width')` | 按容器宽度计算比例，仍受 10%–400% 范围约束 |
| `getZoom()` | 返回 `{ mode: 'fixed' \| 'fit-width', scale: number }` |
| `getCapabilities()` | 查询当前缩放和适应宽度能力 |
| `setWatermark(options \| null)` | 图块就绪后原子替换，返回 `Promise<void>`；`null` 立即关闭 |
| `getWatermark()` | 最后成功应用的归一化配置快照，关闭时为 `null` |
| `on(listener)` | 订阅事件，返回取消订阅函数 |
| `destroy()` | 取消任务、终止 Worker 并释放 DOM；可重复调用 |

`bytes` 支持 `File`、`Blob`、`ArrayBuffer`、`Uint8Array`。`format` 使用不带点的小写值：`doc`、`docx`、`docm`、`xlsx`、`xlsm`、`pptx`、`pdf`。与文件内容冲突的显式格式会报错。

`open()` 结果包含识别后的 `format`、耗时 `durationMs`、`warnings` 和格式相关统计 `stats`。PPT/PDF 提供 `pageCount`，Excel 提供工作表数 `itemCount`；Word 渐进分页，当前不提供最终页数。`open()` 完成表示已解析并挂载，Word 后续分页及 PDF 可视页绘制仍可能继续。

### 事件与错误

| 事件 | 数据与处理建议 |
| --- | --- |
| `statechange` | `state` 为 `idle/loading/ready/error/destroyed`，可用于加载提示 |
| `warning` | `code`、`message`，记录降级或未完整支持的文档特性 |
| `error` | `error.code`、`error.message`；同时覆盖后台 PDF 绘制和 Excel 切表等错误 |
| `watermarkerror` | `operation`、`error`；水印初始化、更新和后台刷新的独立错误 |

| 错误码 | 常见原因 |
| --- | --- |
| `INVALID_SOURCE` | 空文件、数据类型错误、格式冲突、明确损坏的容器或非法缩放比例 |
| `UNSUPPORTED_FORMAT` | 无法识别或不支持的格式 |
| `ASSET_LOAD_FAILED` | Worker、WASM、PDFium 或适配器资源加载失败 |
| `PARSE_FAILED` | 解析器无法读取文件内容 |
| `RENDER_FAILED` | DOM/Canvas 绘制失败，或 Excel 可见区域密度超过保护上限 |
| `INVALID_WATERMARK` | 水印内容/样式非法，或图块超过位图预算 |
| `WATERMARK_FAILED` | 水印绘制、编码/解码或 CSP 失败 |
| `ABORTED` | 文件加载被取消、替换或清空 |
| `DESTROYED` | 实例已销毁，需要新建实例后再打开 |

## 文字水印（0.2.0）

```ts
const viewer = createOfficeViewer(container, {
  watermark: { content: ['内部资料 · 张三（00123）', '运输部 · 2026-10-22'] },
});
viewer.on(event => {
  if (event.type === 'watermarkerror') console.error(event.operation, event.error);
});
await viewer.open({ bytes: file, name: file.name });
await viewer.setWatermark({ content: '内部资料 · 李四', color: '#666', rotate: -25 });
console.log(viewer.getWatermark());
await viewer.setWatermark(null);
```

| 参数 | 默认值与限制 |
| --- | --- |
| `content` | 必填，字符串或字符串数组；按换行展开，1–4 行、每行 ≤128 Unicode 码点，总计 ≤256；首尾空白行移除，行内空格保留 |
| `rotate` | `-25`，有限数值 `-180…180` 度 |
| `opacity` | `0.12`，`0…1`；`0` 仍保留配置 |
| `fontSize` | `14`，`8…72` CSS px |
| `fontFamily` | `sans-serif`，可用于 Canvas 的 CSS 字体列表，≤256 字符；不支持 URL、CSS 变量和全局关键字 |
| `color` | `#000000`，仅 `#RGB` / `#RRGGBB` |
| `gap` | `[120, 80]`，旋转文字包围盒间的水平/垂直空隙，各 `0…512` CSS px |

更新为完整替换，省略字段恢复默认；SDK 复制入参，getter 返回独立快照。空内容报错，关闭使用 `null`。setter 校验失败通过 Promise 拒绝，构造参数非法则在改变 DOM 前同步抛错。初始图块准备期间 getter 为 `null`，`open()` 等待准备完成再挂载。

水印覆盖 Word/PPT/PDF 页面及页间、Excel 工作簿及可点击的工作表标签；不覆盖外部标题和工具栏。字号和间距不随 SDK 缩放，不影响文本选择与滚动。运行时替换不重新下载/解析，也不重置缩放和阅读位置。`clear()` 保留成功配置，重开可复用；取消的更新返回 `ABORTED`，销毁后为 `DESTROYED`。

更新或后台字体/DPR 刷新失败时保留旧水印及文档。初始水印失败会阻止打开，需成功重设或显式关闭；Promise 和事件可携带同一错误对象，按身份去重。图块 DPR 限制为 1–3，边长最多 4096、面积最多 4,194,304 像素，超限不截断或自动缩小。解码超时 5 秒；底层未结束前拒绝新的绘制请求，结束后可手动重试。

CSP 的 `img-src` 需允许 `data:`。水印仅在浏览器生成，不上传身份信息；下载仍为原文件。首期不提供打印、带水印导出或防删除能力。

## 缩放、宽度与剩余高度

工具栏显示的比例应来自 `viewer.getZoom().scale`，还原调用 `viewer.setZoom(1)`。SDK 没有独立的 `zoomchange` 事件，业务在设置比例后更新显示；使用 `fit-width` 时，还需在容器重新测量后刷新比例。今天的附件弹窗采用固定比例按钮，步长为 10%，提供独立还原按钮。

首次加载的比例由业务决定：今天附件弹窗对 Excel 调用 `setZoom(1)`，对 Word/PPT/PDF 调用 `setZoom('fit-width')`；上面的通用页面示例统一从 100% 开始。这些都是宿主策略，可按产品需要调整。

Word/PPT/PDF 缩放内容层，保留原始排版；Excel 只缩放单元格和绘图，工作表标签及滚动视口保持宿主尺寸。PDF 当前放大已有 Canvas，较大比例可能影响清晰度。

SDK 的 `.office-viewer-surface` 和 Excel `.workbook` 使用 `width: 100%`。业务给挂载容器设置 `width: 100%`、`min-width: 0` 即可，无需覆盖内部固定宽度或设置 `100vw`。内部预览位于 Shadow DOM 中，宿主 CSS 应优先管理外层容器。

弹窗外层先分配可用高度，工具栏保持自身高度，预览宿主使用 `flex: 1; min-height: 0; overflow: auto`。如果中间还有 flex 容器，需沿高度链保留 `min-height: 0`，让内容能够收缩并滚动。页面示例中的 `75vh` 可替换为业务弹窗分配的高度；今天附件弹窗采用 `94vh` 外层高度，预览区占剩余空间。

Excel 在明确高度的宿主中逐层填满工作表标签之外的剩余空间；没有明确高度时按内容排版，网格滚动区最大高度为 `63vh`。旧内部变量 `--office-excel-grid-height` 已移除。大表仅挂载当前工作表的可见区域，全部内容通过滚动查看；不要从当前 DOM 提取整份表格数据。

## Vue 2 与附件弹窗

已独立验证的框架组合为 Vue `2.7.16`、Vite `6.4.4`、`@vitejs/plugin-vue2` `2.3.4`。SDK 本身不依赖 Vue；Vue 2.6 或其他构建插件组合需另外验证。

将仓库中的 [OfficePreview.vue](../fixtures/vue2-vite/src/OfficePreview.vue) 复制到业务项目，按 [Vue 2 配置](vue2.md#安装与-vite-配置) 合并 Vite 配置，再通过以下方式使用：

```vue
<template>
  <section class="attachment-preview">
    <input type="file" accept=".doc,.docx,.docm,.xlsx,.xlsm,.pptx,.pdf" @change="selectFile" />
    <div class="attachment-preview__host">
      <OfficePreview ref="preview" :source="source" @ready="onReady" @error="onError" />
    </div>
  </section>
</template>

<script>
import OfficePreview from './OfficePreview.vue';

export default {
  components: { OfficePreview },
  data() { return { source: null }; },
  methods: {
    selectFile(event) {
      const file = event.target.files[0];
      this.source = file ? { bytes: file, name: file.name } : null;
    },
    onReady() { this.$refs.preview.setZoom(1); },
    onError(error) { console.error(error.code, error.message); },
    onDialogOpened() { this.$nextTick(() => this.$refs.preview?.resize()); },
  },
};
</script>

<style scoped>
.attachment-preview { display: flex; flex-direction: column; height: 75vh; min-height: 0; }
.attachment-preview__host { flex: 1; min-height: 0; min-width: 0; }
</style>
```

组件在 `mounted` 创建实例，在 `beforeDestroy` 销毁；实例保存在非响应式字段中。替换整个 `source` 对象切换文件，设为 `null` 清空。需要缩放时调用组件暴露的 `setZoom()`，弹窗完成显示后调用 `resize()`；将 `onDialogOpened` 绑定到所用弹窗组件的打开完成事件。

仓库配套组件还提供 `ready/statechange/warning/error` 事件和 `assetsBaseUrl` 属性，完整说明见 [Vue 2 使用组件](vue2.md#使用组件)。它是可复制的业务封装，公共包入口仍为 `createOfficeViewer()`。

动态创建 Vue 根实例的弹窗若使用 `$t()`，应继承应用上下文或显式注入应用使用的 `i18n` 实例。今天出现的 `Cannot read properties of undefined (reading '_t')` 与动态根实例缺少国际化上下文有关；文件解析 SDK 无需配置 vue-i18n。

### 附件弹窗完整代码

完整示例放在 [examples/attachment-preview](examples/attachment-preview/)，将三个文件一起复制到业务项目，例如 `src/components/AttachmentPreview/`：

| 文件 | 职责 |
| --- | --- |
| [AttachmentPreviewDialog.vue](examples/attachment-preview/AttachmentPreviewDialog.vue) | 弹窗、加载/错误提示、缩放百分比、还原、下载及剩余高度布局 |
| [preview-session.js](examples/attachment-preview/preview-session.js) | 请求附件、创建 SDK 会话、设置初始比例和取消/销毁 |
| [service.js](examples/attachment-preview/service.js) | 服务式打开/关闭，继承 Vue 上下文，替换旧弹窗时释放实例 |

示例依赖业务项目已全局注册的 **Element UI 2.x** `el-dialog` 和 `el-button`；兼容组件库需提供相同的属性、插槽与 `opened/closed/update:visible` 事件。SDK 不包含这两个 UI 组件。尚未注册的项目先安装 `element-ui@2`，并在应用入口中注册：

```js
import Vue from 'vue';
import { Dialog, Button } from 'element-ui';
import 'element-ui/lib/theme-chalk/index.css';

Vue.use(Dialog);
Vue.use(Button);
```

组件式使用适合已有页面或上传组件，通过 `.sync` 管理可见状态：

```vue
<template>
  <div>
    <el-button @click="previewAttachment(attachment)">预览附件</el-button>
    <AttachmentPreviewDialog
      :visible.sync="previewVisible"
      :file-url="previewFile.url"
      :file-name="previewFile.name"
      @error="onPreviewError"
    />
  </div>
</template>

<script>
import AttachmentPreviewDialog from './components/AttachmentPreview/AttachmentPreviewDialog.vue';

export default {
  components: { AttachmentPreviewDialog },
  data() {
    return {
      attachment: { url: '/api/attachments/123/content', name: '培训方案.docx' },
      previewVisible: false,
      previewFile: { url: '', name: '' },
    };
  },
  methods: {
    // 上传组件的 on-preview 回调也可以复用此方法，先映射业务附件字段。
    previewAttachment(file) {
      this.previewFile = { url: file.url, name: file.name };
      this.previewVisible = true;
    },
    onPreviewError(error) { console.error(error); },
  },
};
</script>
```

需要从附件列表点击事件或上传组件回调直接打开时，使用服务入口：

```js
import { openAttachmentPreview } from './components/AttachmentPreview/service.js';

export default {
  methods: {
    async handlePreview(file) {
      const dialog = openAttachmentPreview({
        parent: this,
        i18n: this.$i18n,
        fileUrl: file.url,
        fileName: file.name,
        watermark: { content: ['内部资料 · 张三', '运输部'] },
      });
      dialog.$on('error', error => console.error(error));
      dialog.$on('download-error', error => console.error(error));
      dialog.$on('watermark-error', error => console.error(error));
      // 需要切换显示身份时，等待真实图块提交；不会重新加载附件。
      await dialog.setWatermark({ content: '内部资料 · 李四' });
    },
  },
};
```

`fileUrl` 是可请求的附件地址，`fileName` 应携带原扩展名；无文件名时示例从 URL 路径推断。服务入口同一时间只保留一个弹窗，再次打开会销毁上一个；它返回 Vue 包装实例，可监听 `ready/error/download-error/watermark-error`，通过 `setWatermark()` / `getWatermark()` 更新和读取配置。水印 setter 等待实际提交，失败回滚期望配置；关闭会拒绝尚未完成的更新。关闭按钮通过状态和关闭动画完成清理；传入 `parent` 后，页面销毁会自动释放所属弹窗。也可导入并调用 `closeAttachmentPreview()` 立即关闭当前弹窗；未传 `parent` 时应在业务卸载处显式调用。示例默认使用中文文案，不依赖 `$t()`；业务替换为国际化文案时，传入 `parent` 或同一应用的 `i18n` 实例。

组件属性及服务参数如下，函数签名中的 `signal` 均为 `AbortSignal`：

| 参数 | 默认值 / 约定 |
| --- | --- |
| `visible` | 组件式必传，通过 `.sync` 更新；服务式自动打开 |
| `fileUrl` / `fileName` | 附件 URL / 原文件名 |
| `title` | 无文件名时的标题，默认“附件预览” |
| `assetsBaseUrl` | 默认空，使用包内资源；自定义时传公开引擎目录 |
| `watermark` | 默认 `null`；文字水印对象（0.2.0），参数见上文；组件按对象替换更新 |
| `fetchFile({ fileUrl, fileName, signal })` | 默认同源 Cookie 的 `fetch`；自定义函数返回 `Promise<Blob \| ArrayBuffer \| Uint8Array>`，同时用于预览和默认下载 |
| `downloadHandler({ fileUrl, fileName, signal })` | 可选，自行完成原文件下载并返回 Promise；用于审计或独立下载接口 |
| `parent` / `i18n` | 仅服务式参数，提供 Vue 应用上下文 |

需 Token、跨站 Cookie 或统一错误处理时，通过 `fetchFile` 对接业务请求层。例如 Axios 1.x 可返回文件数据并传递取消信号：

```js
async function fetchFile({ fileUrl, signal }) {
  const response = await api.get(fileUrl, { responseType: 'blob', signal });
  return response.data; // api 为业务 Axios 实例；若拦截器已解包，直接返回 response。
}
```

将 `fetchFile` 传给 `openAttachmentPreview({ ..., fetchFile })`，或绑定组件的 `:fetch-file="fetchFile"`。自定义 `downloadHandler` 同样需要将 `signal` 传入请求层，并在保存文件前检查 `signal.aborted`；示例可以取消默认请求，但无法中断忽略该信号的业务回调。较早的 Axios 版本需自行桥接取消机制。

首次预览 Excel 为 100%，Word/PPT/PDF 按宽度适配；工具栏只提供缩小、放大和还原到 100%，比例步长为 10%，范围为 10%–400%。弹窗高度为 `94vh`，宿主宽度为 `100%`，通过 flex 占剩余高度，没有 `507px` 固定宿主高度或 `100vw` 覆盖。预览失败仍可下载原文件，下载错误单独显示。关闭、切换附件和卸载时，同时取消网络请求、释放 SDK 会话与 Worker，并忽略过期请求结果。

此示例覆盖上述七种文档格式，图片和音视频沿用业务原有预览入口。这里的源码属于可复制的业务组件，不是 SDK 的额外 npm 导出，也不依赖基础 `OfficePreview.vue`。

这份弹窗示例已在 Vue `2.7.16`、Element UI `2.15.3`、Vite `6.4.4` 开发模式和 Chrome 下验证 Word/Excel/PPT/PDF 样例预览、缩放与还原、原文件下载、慢请求替换、预览失败后的下载以及关闭/页面销毁时的 Worker 释放。其他兼容 UI 组件库需按相同交互另外验证。

## 原文件下载

顶部下载按钮由宿主业务提供，沿用现有附件下载接口、鉴权和文件名。已持有原始 `Blob` 时，也可以在业务点击处理函数中使用下面的辅助方法：

```ts
export function downloadOriginal(file: Blob, name: string) {
  const url = URL.createObjectURL(file);
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
```

下载交付的是原始附件。SDK 当前没有文档编辑、格式转换、保存修改、搜索、打印或导出 API；PDF 通过 Canvas 展示，未提供文字选择层。大文件下载可直接使用后端下载地址，避免为下载再复制一份文件数据。原有上传类型、大小和附件访问权限仍由业务系统校验。

## 静态资源部署与许可

普通 Vite 项目优先使用包内默认资源，由生产构建输出 Worker/WASM。应用部署在子路径时，按业务实际路径配置 Vite `base`。

需要固定资源目录时，将四个适配器包的 `dist/assets/` 内容合并复制到公开目录，保留 `word/`、`excel/`、`ppt/`、`pdf/` 子目录及 PDFium/glue 文件。创建实例时指定最终访问地址：

```ts
import { createOfficeViewer } from '@office-view-suite/viewer';

export function createStaticAssetViewer(container: HTMLElement) {
  return createOfficeViewer(container, {
    assets: { baseUrl: '/static/office/' },
  });
}
```

`/static/office/` 应与部署后的目录一致。部署在子路径时使用包含该前缀的地址；不要将业务附件 URL 作为引擎资源地址。各适配器提供 `dist/asset-manifest.json`，可用于核对资源路径及哈希。

服务端需为 JavaScript 和 WASM 返回正确的 MIME，资源缺失时应返回实际错误，避免将 SPA 的 `index.html` 当作 Worker/WASM 返回。Worker 入口通常保持同源；CDN、跨域 WASM、自定义 Worker 和 CSP 配置见 [Worker 和 WASM 部署](sdk.md#worker-和-wasm-部署)。

项目原创代码采用 [Apache-2.0](../LICENSE)，允许商业使用及集成到闭源应用。分发或部署时保留安装包中的 `dist/LICENSE`、各适配器的 `dist/THIRD_PARTY_NOTICES.md` 与 `dist/licenses/`。仅靠 Vite 引用资源不会自动复制全部声明，宿主需单独部署，并在“关于 / 开源许可”等位置提供访问。PDFium 等依赖继续遵循各自许可证，详见 [许可部署说明](sdk.md#第三方许可随包部署)。

## 常见问题与效果边界

| 现象 | 排查与处理 |
| --- | --- |
| 开发模式请求 `node_modules/.vite/deps/assets/...` 失败 | 六个 SDK 包全部加入 `optimizeDeps.exclude`，重启并刷新旧缓存 |
| Worker/WASM 返回 HTML 或提示 MIME 错误 | 检查实际资源 URL、Vite `base`、静态目录和 SPA 回退配置 |
| 弹窗打开后空白、宽度不对 | 等待容器可见且宽度可测量，再创建或调用 `resize()` |
| Excel 只占部分高度、无法填满 | 检查外层可用高度和中间 flex 容器的 `min-height: 0`；删除旧固定高度覆盖 |
| 单元格显示过大或过小 | 先查看真实缩放比例，调用 `setZoom(1)`；Excel 保留原始行列尺寸，不按内容数量压缩整张表 |
| Word 灰色虚线与源文件实线不同 | 灰色虚线可能是无边框单元格的辅助网格，需核对原文件边框定义 |
| 切换附件后出现旧文件 | 同时取消业务下载和 SDK 会话，参考远程附件示例 |
| 同一错误提示两次 | 同时处理 Promise 和事件时按错误对象去重；忽略正常替换产生的 `ABORTED` |
| Vue 动态弹窗调用 `$t()` 报 `_t` 错误 | 为根实例提供同一应用的 `i18n` 上下文 |

预览依赖浏览器可用字体和解析器覆盖范围。Word 复杂分页、浮图环绕、跨页合并表格等布局，PPT 部分阴影、图表、SmartArt 和三维效果仍有限制；PDF 高比例缩放可能降低清晰度。需要原稿验收时，应使用同一源文件由 Word/WPS/PowerPoint 导出的结果逐页核对，不能将旧渲染器截图一致视为原稿完全还原。

当前验证以现代 Chrome 为主，需支持 Worker、WebAssembly、Shadow DOM、ResizeObserver；PDF 使用 Canvas 和 IntersectionObserver。SSR、UMD、Safari/Firefox 及独立 Webpack/Rollup 接入未列为已验证交付范围。具体未解决的视觉差异见 [预览差异跟踪](known-preview-differences.md)。
