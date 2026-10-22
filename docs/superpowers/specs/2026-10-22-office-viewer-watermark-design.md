# Office Viewer SDK 水印修订设计

日期：2026-10-22。代码基线：`667d52d`，已发布 SDK：`0.1.2`。

状态：已实施于源码 `0.2.0`，尚未发布。公共 SDK、Vue 2 组件、附件 session/弹窗/服务已接入；已发布 `0.1.2` 不提供水印。本设计替代此前对话中的初版方案，验收记录见末节。

## 1. 目标与范围

在公共 `createOfficeViewer()` 入口增加文字平铺水印，覆盖 `.doc/.docx/.docm/.xlsx/.xlsm/.pptx/.pdf`。用户姓名、工号、部门、时间由宿主拼接后传入；库在浏览器内完成绘制，不读取登录状态，不上传水印内容。

首期支持多行文本、角度、透明度、字号、字体、颜色、间距，以及运行时开启、替换和关闭。水印默认关闭，保持文档排版、缩放、滚动和文件数据不变。使用现有 Canvas 能力，不增加运行时依赖。

本次交付公共 SDK、仓库内 Vue 2 配套组件、附件弹窗示例及文档。业务仓库使用新版包时单独更新接入。直接使用格式 adapter 的调用方不自动获得水印；水印能力由公共 viewer 提供。

原文件下载仍交付原始附件。图片水印、带水印文件导出、打印水印和防删除机制不列入首期。前端覆盖水印用于预览标识，不能保证浏览器端不可移除。

## 2. 审查问题与修订决策

| 审查问题 | 修订后的实施约定 | 验收依据 |
| --- | --- | --- |
| 覆盖范围和尺寸更新不明确 | 独立覆盖层；按真实内容范围测量；观察内容增长；测量排除覆盖层 | 后续分页、横向滚动、缩小后无漏盖和多余滚动 |
| 绘制失败可能被当成成功 | 异步更新返回 Promise；新图块就绪后原子替换；独立水印错误事件 | 首次失败明确拒绝，更新失败保留旧图块 |
| API、生命周期与竞态不完整 | 明确输入限制、配置快照、实例级持久配置、独立更新序号 | 清空后重开、连续更新、关闭和销毁时旧任务不回流 |
| 服务式弹窗只能初始化透传 | session、组件、服务包装实例均提供水印更新方法 | 更新不重新下载、解析、创建 Worker 或重置缩放 |
| 图块可能裁切、模糊或过大 | 按旋转边界计算尺寸；限制文本与位图；处理字体和 DPR 变化 | 长中文、多行、斜体边界、高 DPR、延迟字体 |

## 3. 当前实现约束

- [viewer facade](../../../packages/viewer/src/index.ts) 的 `.office-viewer-content` 对 Word/PPT/PDF 使用 CSS `zoom`；Excel 在 renderer 内缩放网格。水印不能进入该缩放层。
- adapter 的 `render()`、facade 的 `clearRendered()` 和 `renderCurrent()` 会清空或搬运 surface ShadowRoot 中的节点。水印必须由 facade 独立持有。
- `.office-viewer-host` 当前只有宽度和 `overflow: auto`，没有统一的固定高度。Word/PPT/PDF 可能由外部宿主滚动；Excel 的 `.grid-wrap` 自行滚动，且区分有界高度和自动高度。
- 当前 facade 只观察宿主尺寸；Word 渐进分页、图片加载与字体替换可能在宿主尺寸不变时改变内容范围。
- [附件弹窗服务](../../examples/attachment-preview/service.js) 捕获创建时的参数，返回 Vue 包装实例；[session 封装](../../examples/attachment-preview/preview-session.js) 目前只转发缩放等方法。

实施不改变宿主容器的样式、已有子节点或 ShadowRoot。水印新增结构、样式、监听和缓存均归 SDK 所有。

## 4. 展示与几何约定

### 4.1 覆盖语义

水印覆盖 SDK 的预览区域。Word/PPT/PDF 包括页面和页间空隙；Excel 包括工作簿展示区域及工作表标签。标签仍可点击。弹窗标题、下载和缩放工具栏属于 SDK 外部，不在覆盖范围内。首期不提供“只盖纸张”“排除工作表标签”等覆盖模式。

水印字号、图块尺寸和间距均为 CSS 像素，不跟随 SDK 文档缩放。这里的承诺不包括宿主给整个页面施加的 CSS transform 或浏览器页面缩放。

水印随既有滚动持续重复，不承诺图案固定在屏幕上的同一位置。短文档下方由宿主额外分配的空白不要求覆盖；有界 Excel 按其实际工作簿高度覆盖。

### 4.2 插入位置

```text
调用方 container（原样保留）
└─ office-viewer-host（SDK 所有，保持当前滚动职责）
   ├─ office-viewer-surface
   │  └─ ShadowRoot → office-viewer-content → 格式渲染结果
   └─ office-viewer-watermark（绝对定位、独立 ShadowRoot）
      └─ 图块平铺层
```

开启水印时，在 SDK 自有 host 建立局部定位和层叠上下文。surface 与水印分别位于局部内容层和覆盖层；水印高于文档内部冻结单元格、图片和形状，不使用影响整页的超大 z-index。新增定位/层叠样式要记录原值，关闭水印时恢复。

水印节点使用 `pointer-events: none`、`user-select: none`、`aria-hidden="true"`，不设置可聚焦属性。装饰性文字绘入图块，不进入文档正文或可复制文本。

### 4.3 范围测量和更新

实现独立的 `measureWatermarkBounds()`，从 surface 和 content 的实际可滚动范围、视觉边界及当前缩放推导覆盖矩形。不得直接读取包含水印节点的 host `scrollWidth/scrollHeight` 作为唯一真源，也不得把未缩放的尺寸当成视觉尺寸。

具体规则：

1. Word/PPT/PDF 使用内容在当前 SDK 缩放下的可访问范围，覆盖横向溢出和渐进增加的页面；Excel 使用工作簿外框，内部网格滚动不要求生成整张超大表的水印区域。
2. 范围测量只读文档节点；覆盖层采用绝对定位，不参与正常流，不改变可访问的滚动范围。内容缩小时，覆盖范围必须同步收缩。
3. `ResizeObserver` 观察 surface/content，而不只观察外部宿主；`open()` 挂载、重渲染、`setZoom()`、`resize()` 和 Excel 切表后显式请求几何更新。
4. 对固定外框内的溢出变化，使用受限的内容 `MutationObserver` 作为补充。只观察当前文档子树，合并为一帧一次测量；不观察水印子树、不逐单元格绑定 observer、不调用格式 renderer 的重排来更新水印。
5. 所有测量经 `requestAnimationFrame` 合并；矩形未变化不写样式。字体和图像引起的变化也走同一路径。重渲染时解绑旧目标，绑定新目标。
6. 隐藏容器的零尺寸只推迟几何展示，不将配置判为失败。重新显示后通过 observer 或宿主 `resize()` 恢复覆盖。

先用真实浏览器验证此几何模型，再扩展 API 封装。必须覆盖有界高度、自动高度、横向溢出和恢复显示；不能通过改变所有格式的滚动所有权来绕过问题。

## 5. 公共 API

下列声明是对现有类型的增量；原有方法保持原义。

```ts
export interface OfficeWatermarkOptions {
  content: string | readonly string[];
  rotate?: number;
  opacity?: number;
  fontSize?: number;
  fontFamily?: string;
  color?: string;
  gap?: readonly [number, number];
}

export interface OfficeWatermarkConfig {
  readonly content: readonly string[];
  readonly rotate: number;
  readonly opacity: number;
  readonly fontSize: number;
  readonly fontFamily: string;
  readonly color: string;
  readonly gap: readonly [number, number];
}

// OfficeViewerOptions 增加：
// watermark?: OfficeWatermarkOptions | null;

// OfficeViewer 增加：
// setWatermark(value: OfficeWatermarkOptions | null): Promise<void>;
// getWatermark(): OfficeWatermarkConfig | null;

// OfficeViewerError.code 增加：
// 'INVALID_WATERMARK' | 'WATERMARK_FAILED';

// OfficeViewerEvent 增加：
// { type: 'watermarkerror'; operation: 'initialize' | 'update' | 'refresh';
//   error: OfficeViewerError };
```

### 5.1 输入和默认值

| 参数 | 默认值 | 校验与归一化 |
| --- | --- | --- |
| `content` | 无，启用时必填 | 字符串按换行拆分；数组元素也按换行展开；保留行内空格，去除首尾空白行；归一化后 1–4 行，每行最多 128 个 Unicode 码点，总计最多 256 个；必须含非空白字符 |
| `rotate` | `-25` | 有限数值，范围 `-180` 至 `180`，单位度 |
| `opacity` | `0.12` | 有限数值，范围 `0` 至 `1`；`0` 表示透明，不等价于关闭 |
| `fontSize` | `14` | 有限数值，范围 `8` 至 `72`，单位 CSS px |
| `fontFamily` | `sans-serif` | 非空字体族列表，最多 256 字符；通过 CSS 属性及 Canvas 字体简写校验；不接受外部 URL、CSS 变量或全局关键字 |
| `color` | `#000000` | 首期限定 `#RGB` 或 `#RRGGBB`，归一化为六位色值；透明度统一由 `opacity` 控制 |
| `gap` | `[120, 80]` | 两个有限数值，范围各为 `0` 至 `512`；单位 CSS px，分别表示相邻旋转文本包围盒间的水平/垂直空隙 |

空字符串、空数组不是关闭命令；关闭统一使用 `null`。超过限制返回错误，不静默截断文字、缩小字体或修改传入数据。

`setWatermark()` 完整替换配置，省略样式字段恢复默认值。调用时复制内容和间距数组，后续修改入参不影响 SDK。`getWatermark()` 返回归一化配置的独立快照，调用方无法通过修改返回数组改变当前水印。

### 5.2 完成语义

`setWatermark()` 始终返回 Promise，参数校验失败通过 `INVALID_WATERMARK` 拒绝，不采用部分同步抛错、部分异步拒绝的混合方式。当前配置和图块在新图块准备完成之前保持不变；成功后同步提交配置与图块，Promise 才完成。

`getWatermark()` 返回最后成功提交的配置；尚未成功启用或已关闭时返回 `null`。该结果表示图块可用，不承诺容器此刻可见或已经完成浏览器屏幕绘制。

构造参数中的 `watermark` 在创建前做同步输入校验，非法时在修改 DOM 之前抛出 `INVALID_WATERMARK`。合法配置作为初始待应用任务，在微任务中开始准备图块；内部接住拒绝，避免未处理的 Promise。初始化成功前 `getWatermark()` 为 `null`。`open()` 等待当前有效的水印任务后再展示文档；若需显式等待而不打开文件，使用 `setWatermark()`。

示例：

```ts
const viewer = createOfficeViewer(container, {
  watermark: { content: ['内部资料 · 张三（00123）', '2026-10-22'] },
});

viewer.on(event => {
  if (event.type === 'watermarkerror') reportWatermarkError(event.error);
});

await viewer.open({ bytes, name: '培训方案.docx' });
await viewer.setWatermark({ content: ['内部资料 · 李四（00456）'] });
const applied = viewer.getWatermark();
await viewer.setWatermark(null);
```

`bytes`、`container` 和 `reportWatermarkError` 由宿主提供。Promise 拒绝与事件可能携带同一错误对象，宿主需按对象身份去重。

## 6. 事务更新、生命周期与错误

### 6.1 实例与文档会话

水印属于 viewer 实例，更新序号独立于文档解析会话。只有最新有效的水印任务可以提交配置、图块或错误。

| 操作 | 配置和任务行为 | 展示和资源行为 |
| --- | --- | --- |
| 创建，未配置水印 | 已应用配置为 `null` | 不创建水印 DOM、图块或专用 observer |
| 创建并配置水印 | 异步准备初始配置 | 文档挂载前完成图块；准备失败不静默降级 |
| `setWatermark(options)` | 校验后递增更新序号，取消上一有效准备任务；成功后替换配置 | 保留旧图块直至新图块可用；不重新请求或解析文档 |
| `setWatermark(null)` | 立即使旧任务失效并清空已应用配置；返回已完成 Promise | 当次调用立即移除覆盖层，释放图块及专用监听 |
| `open()` 替换文件 | 保留实例级配置，等待当前有效准备任务 | 跟随当前文档重绑几何；旧文档回调不能修改新覆盖层 |
| `clear()` | 保留已应用配置；取消当前未完成更新，回退到已应用配置 | 移除展示及文档几何监听，可保留一个已提交图块供重开复用 |
| `destroy()` | 所有任务以 `DESTROYED` 结束，清空配置 | 释放图块、URL、observer、字体/DPR 监听和 RAF；不再提交任何结果 |

输入校验（包括按当前可用字体测量的图块预算检查）在替换有效任务之前完成，失败不取消原本有效的更新任务。有效的新更新、关闭或清空使旧更新 Promise 以 `ABORTED` 尽快结束；不等待不可取消的底层字体/图片 Promise。底层任务晚到时只释放它持有的临时资源。

`open()` 等待期间如果水印任务被另一有效更新取代，应继续等待最新任务；不得把内部更新的 `ABORTED` 当作文件打开失败。文件的 `AbortSignal` 和 viewer 销毁仍按现有规则终止 `open()`。

同时更换附件和水印时，Vue 包装先清空旧内容，等待对应 `setWatermark()` 成功，并确认 source/配置版本仍有效后，再开始新文件 `open()`。如果新水印失败，保持空预览并报告错误；不能用保留的旧图块打开新附件。创建新 session 的弹窗则将对应水印作为构造选项，使其首次 `open()` 遵循同样的展示前置条件。

销毁后 `setWatermark()` 拒绝 `DESTROYED`，`getWatermark()` 返回 `null`。`clear()` 对已销毁实例仍按现有逻辑无操作。

### 6.2 错误和失败后的行为

| 情况 | API 与事件 | 文档状态 |
| --- | --- | --- |
| 参数非法 | 构造阶段同步抛出；`setWatermark()` 拒绝 `INVALID_WATERMARK`；不再额外发送水印事件 | 原配置、当前准备任务和文档保持原样 |
| 构造选项的初始图块失败 | 记录 `WATERMARK_FAILED` 并发送一次 `watermarkerror` | 后续 `open()` 在挂载内容前拒绝，直至成功设置或显式关闭 |
| 未有水印时显式 setter 绘制失败 | setter 拒绝 `WATERMARK_FAILED` 并发送一次 `watermarkerror`，已应用配置仍为 `null` | 已打开文档保持原样；没有构造选项的初始失败时，不额外设置隐藏的打开阻断状态 |
| 有旧图块时更新失败 | setter 拒绝并发送一次 `watermarkerror`，恢复旧的已应用配置 | 旧图块继续显示，文档 `ready` 状态保持 |
| 字体/DPR 后台刷新失败 | 每次刷新尝试发送一次 `watermarkerror`，保留旧图块，不无限重试 | 文档状态保持；下次相关变化或显式更新可重试 |
| 更新被取代、关闭、清空或销毁 | 对应 Promise 拒绝 `ABORTED/DESTROYED`，不发送故障提示事件 | 按生命周期表执行 |

`open()` 被初始水印错误阻止时，仍遵循打开失败的现有契约：Promise 拒绝、文档进入 `error`、发送普通 `error` 事件，但错误码是 `WATERMARK_FAILED`，不能改写成 `PARSE_FAILED`。已有 `watermarkerror` 与该普通事件复用同一错误对象，避免宿主无法去重。运行中的水印更新/刷新失败只发 `watermarkerror`，不走文档失败通路。

已发生的构造选项初始化失败不能由 `clear()` 隐式关闭；须成功设置水印或调用 `setWatermark(null)` 后解除。若 `clear()` 发生在初始图块尚未完成时，则取消该任务并保留已校验的初始意图，下一次 `open()` 重新准备；这与已有成功配置时取消未完成更新、回退到已应用配置的行为区分开。

启用水印不是文档访问控制开关。对已经展示且此前没有水印的文档，首次动态启用失败会明确通知宿主，但不自动销毁文档；需要“展示前必须带水印”的应用应在创建时配置，或等待 `setWatermark()` 成功后再打开文件。

## 7. 图块、字体与资源预算

### 7.1 图块生成

1. 按规范化字体和字号测量每行；使用字形实际边界补偿超出 advance width 的部分，覆盖中文、英文与字体回退。
2. 行高固定为 `1.4 × fontSize`；以整个多行文本块为中心旋转。角度为 `θ` 时，基础包围盒可用 `|w cosθ| + |h sinθ|` 和 `|w sinθ| + |h cosθ|` 计算，再留每侧至少 2 CSS px 的抗锯齿余量。
3. 图块 CSS 宽高为旋转包围盒加上 `gap`，文本居中，使相邻图块的文字保持约定间距。
4. DPR 取 `min(3, max(1, devicePixelRatio))`，物理像素向上取整；CSS `background-size` 固定为逻辑尺寸，避免高分屏改变水印密度。
5. 单个图块物理宽高均不得超过 4096 px，面积不得超过 4,194,304 像素（RGBA 约 16 MiB）。显式配置的测量超限以 `INVALID_WATERMARK` 拒绝；该检查必须发生在创建大 Canvas 之前，保留旧水印。已有配置因字体/DPR 变化而在后台刷新时超限，则报告 `WATERMARK_FAILED` 并保留旧图块。
6. Canvas 生成 PNG data URL，预加载并等待图片解码成功后原子替换背景。每个实例至多持有一个已提交图块和一个候选图块；滚动与普通尺寸变化不重新编码图块。

不创建整份长文档或整张工作表大小的 Canvas，不为每一页或单元格生成水印 DOM。平铺层本身可以覆盖较大内容范围，是否产生异常绘制开销需通过长文档/大表浏览器验证。

### 7.2 字体与 DPR

首次生成使用浏览器当前可用字体，不无限等待网络字体。监听 `document.fonts.loadingdone`，在有效配置未变化时重新测量、生成并原子替换；旧字体任务不得覆盖更新后的用户名或已经关闭的水印。

通过分辨率媒体查询及窗口 resize 检查 DPR 变化，变化后合并刷新。普通文档缩放不触发字体/DPR 图块重建。关闭水印时解绑监听；重新开启时重建必要监听。

图块生成和解码串行，额外请求只保留最后一份待处理配置。旧任务被取代时立即结束调用方的 Promise，但其在途解码仍占用串行槽位；下一份候选须等底层任务结束或确认取消完成后启动，不能把移除引用等同于解码结束。

单次图片解码设置 5 秒超时。超时后以 `WATERMARK_FAILED` 结束当前有效请求、保留旧图块；若底层任务尚未结束，暂停生成器并明确拒绝新绘制请求，避免反复超时累积位图。待底层任务结束后恢复接受新请求，不自动重放已拒绝的请求。关闭和销毁仍立即生效，晚到结果只能清理资源。浏览器内部回收时机不作为 SDK 的同步释放承诺。

### 7.3 部署要求

PNG data URL 沿用现有文档 data 图片的 CSP 要求，部署说明应明确 `img-src` 需允许 `data:`，并沿用现有样式策略。图块预加载因策略失败时报告 `WATERMARK_FAILED`，不记录水印正文。

首期固定使用 data URL，不自动尝试外部 URL 或上传图片。配置、图块和 Canvas 引用在替换/关闭/销毁后释放；若实现阶段改用 Blob URL，必须同步修改此约定并补充 `revokeObjectURL` 和 CSP 验收。

## 8. Vue 2 与附件弹窗

### 8.1 组件和 session

- `OfficePreview.vue` 与 `AttachmentPreviewDialog.vue` 增加 `watermark: Object | null` 属性，默认 `null`。
- 组件采用对象替换触发更新，不使用深度 watcher；数组或字段原地修改不属于自动更新契约。
- 独立 watcher 调用 SDK/session 的 `setWatermark()`，不能调用 `queuePreview()`、`openSource()` 或重置缩放和滚动位置。
- `preview-session.js` 在创建时转交 `watermark`，并转发 `setWatermark()`、`getWatermark()`。SDK 实例继续保存在非响应式字段中。
- 组件发出独立 `watermark-error`，使用独立的水印错误提示状态；不复用现有 `errorMessage` 来禁用缩放或下载。仅初始配置阻止 `open()` 时进入预览错误通路。
- source 和 watermark 在同一轮变更时合并调度，按第 6 节先清空旧内容、等待对应水印成功，再打开新 source；每个 viewer/session 的身份和水印更新序号均需校验。

### 8.2 服务式入口

`openAttachmentPreview({ ..., watermark })` 支持初始化。保留当前返回 Vue 包装实例的行为，给该实例增加：

```ts
// 返回的 Vue 包装实例实现以下增量方法。
interface AttachmentPreviewWatermarkMethods {
  setWatermark(value: OfficeWatermarkOptions | null): Promise<void>;
  getWatermark(): OfficeWatermarkConfig | null;
}
```

包装实例用自己的响应式状态保存期望配置，并通过子组件方法/session 更新。服务 setter 的 Promise 必须等待子组件成功提交，不能在修改 prop 后立即完成；更新失败恢复已应用配置，避免后续重新挂载时意外重试失败配置。同一更新不能由方法和 watcher 各执行一次。

若弹窗懒挂载尚未完成，服务缓存最新配置，首次创建 session 使用该配置，返回的 Promise 在应用成功后完成。关闭或所属页面销毁后，未完成 setter 拒绝并释放回调；不创建已关闭弹窗的 viewer。

```js
const dialog = openAttachmentPreview({
  parent: this,
  fileUrl: file.url,
  fileName: file.name,
  watermark: { content: [`内部资料 · ${this.userName}`, this.departmentName] },
});

dialog.$on('watermark-error', error => reportWatermarkError(error));
await dialog.setWatermark({ content: ['内部资料 · 李四（00456）'] });
```

普通属性绑定由父组件持有期望值，组件在失败时报告错误并保留已应用水印，不直接修改父组件 prop。服务式入口由包装实例持有期望值，可以在失败时恢复该状态。两种方式共用应用过程，不并行维护两份 SDK 配置。

## 9. 文件划分与实施顺序

| 文件/范围 | 职责 |
| --- | --- |
| `packages/viewer-core/src/index.ts` | 水印类型、配置、错误码和事件契约 |
| `packages/viewer/src/watermark.ts`（新增） | 输入校验、图块绘制、原子更新、独立任务序号与资源释放 |
| `packages/viewer/src/watermark-layout.ts`（新增） | 局部覆盖层、范围测量、几何观察与 RAF 合并 |
| `packages/viewer/src/index.ts` | 初始配置、公开方法、open/clear/destroy 联动、事件及类型导出 |
| `fixtures/vue2-vite/src/OfficePreview.vue` | 组件属性、方法、事件与 source 更新顺序 |
| `docs/examples/attachment-preview/*` | session 转发、弹窗属性、服务式更新入口和错误提示 |
| `tests/sdk-watermark*.mjs`（新增） | 配置、错误、更新顺序、生命周期测试 |
| `scripts/*` 与消费端 fixture | 打包、真实浏览器水印与兼容性验证 |
| `docs/usage.md`、`docs/sdk.md`、`docs/vue2.md`、`README.md` | 发布后 API、示例、CSP 和边界说明 |

实施顺序：

1. 先验证独立覆盖层的几何模型，记录四格式开启/关闭前后的尺寸、滚动与截图；固定宿主、自动高度、横向溢出和 Word 后续分页都通过后继续。
2. 实现配置校验、Canvas 图块与异步更新事务，覆盖取消、首次失败、保留旧图块和资源预算。
3. 接入 facade 的公开 API 与生命周期，补齐初始水印失败阻止 `open()` 的处理。
4. 接入 Vue 2 组件和附件弹窗，验证动态更新不重新下载/解析、不增加 Worker。
5. 执行安装包、生产构建和视觉回归，更新使用文档，再按仓库发布流程统一调整六包版本。

六个 SDK 包源码版本统一为 `0.2.0`，尚未发布。常规调用默认行为兼容；使用事件穷尽检查或自行实现 `OfficeViewer` mock 的 TypeScript 消费端需更新声明。

预计无需修改 Rust/WASM；只有实际调整解析器或其输入契约时才重新评估 WASM 构建。新能力通过已发布包入口验证，不能仅使用源码 alias。

## 10. 验收矩阵

| 类别 | 必测场景 | 通过条件 |
| --- | --- | --- |
| 默认兼容 | 未配置水印；开后关闭 | 无额外水印节点/监听；既有正文、页数和滚动几何保持 |
| 四格式 | Word 多页、Excel 多表及冻结区、PPT 多页、PDF 懒绘制 | 所有已展示内容有覆盖；重绘不丢失；Excel 标签可点击 |
| 容器 | 有界高度、自动高度、隐藏再显示、弹窗 resize | 恢复后覆盖正确；不侵入标题与工具栏 |
| 几何 | 横向/纵向滚动；10%、100%、400%；fit-width/还原 | 无后续页面或右侧漏盖；缩小时范围回收；水印 CSS 字号保持 |
| 内容增长 | Word idle 分页、图片加载、字体重排 | 宿主尺寸不变时仍更新覆盖范围；无 ResizeObserver 循环 |
| 初始失败 | 无 Canvas 上下文、编码/解码失败、CSP 阻止图块 | 初始化明确报错；open 不展示无水印文档；成功重设/null 可恢复 |
| 更新失败 | 已有水印时使用非法配置或模拟绘制失败 | 旧配置/图块保留，文档 ready、滚动与缩放保持 |
| 并发 | A 慢 B 快；更新后关闭；更新后 clear/destroy | 仅最新有效任务提交；旧 Promise 及时结束；无旧图块回流 |
| API | 完整替换、null、快照拷贝、清空后重开 | 默认值与生命周期符合本文；无入参/返回值引用泄漏 |
| 图块 | 长中文、多行、边界字符、旋转、延迟字体、DPR 1/2/3 | 无截断，密度不变；超限在分配大 Canvas 前拒绝 |
| 性能 | 长 Word、大 Excel、连续滚动/更新、多实例 | 不生成文档大小位图；滚动无重复编码；每帧最多一次几何测量/写入；无持续堆积任务 |
| Vue 2 | 属性替换、服务 setter、source 与水印同时变化、路由销毁 | 无重复下载/解析/Worker；Promise 反映实际应用结果；资源释放 |
| 包和部署 | 独立 tarball 消费、开发/生产、默认/自定义引擎资源 | 类型和水印模块完整；Worker/WASM 路径与原有能力正常 |

几何断言需比较开启前后 `clientWidth/clientHeight/scrollWidth/scrollHeight` 和滚动位置，允许浏览器取整导致的最多 1 CSS px 差异；更大的变化必须解释并修复，不能直接更新基线。

生命周期测试使用可控的异步绘制与字体任务验证真实顺序，不能只检查函数是否被调用。浏览器测试核对水印像素和截图，并验证文本选择、滚动、下载和关闭交互。性能记录包括图块生成次数、几何更新次数、文档/水印节点数及清理后的监听状态，不承诺跨机器统一帧率。

验证命令：

```sh
pnpm build:sdk
pnpm typecheck
pnpm test
pnpm test:packages
pnpm test:browser
pnpm test:vue2
pnpm test:watermark:layout
pnpm test:watermark
pnpm test:attachment
pnpm test:visual
```

浏览器脚本按现有约定提供 `PLAYWRIGHT_MODULE`。水印开启场景使用新增断言/截图，水印关闭场景继续对比已有基线；已知 Word/Excel 历史差异仍按 [预览差异跟踪](../../known-preview-differences.md) 记录，不能把水印验收通过写成 Office 还原度全部通过。

实现阶段若变更覆盖语义、失败处理、资源形式或公开 API，需要同步更新本设计和使用文档。

## 11. 实施与验收记录

实现保持独立 ShadowRoot 覆盖层、PNG data URL 图块和串行解码。`clear()` 释放监听并保留已应用图块，重开前重新测量和生成，避免清空期间字体或 DPR 变化后永久复用旧图块；刷新失败保留缓存并报告 `watermarkerror`。准备中的字体/DPR 变化合并为对最新有效身份的后续刷新。

- 几何浏览器验证通过 33 个场景：四格式、有界/自动高度、10%/100%/400% 与缩回、横向滚动、隐藏恢复和内容增长/回收，开关前后几何差不超过 1 CSS px。
- 独立安装包的水印验证通过 10 组，含四格式共 16 个缩放场景、真实 PNG 像素、Canvas 字体校验、DOM/Worker 保持、生命周期、错误对象身份、失败回退、字体/DPR 和 CSP；四格式截图已检查。
- Vue 2 开发/生产 × 默认/固定资源目录通过 56 项；Element UI 2.15.3 附件服务开发/生产通过 8 项，验证懒挂载、独立错误、身份切换、无重复下载以及销毁取消。
- 构建、TypeScript、150 项 Node 回归与独立 tarball 类型/生产构建校验通过；原有浏览器回归 22 项通过。旧视觉基线仍有已记录的 6 份样例差异，9 组 Word 缩放分页检查通过，没有更新基线来消除失败。

浏览器证据位于忽略目录 `output/playwright/{watermark-layout,watermark,vue2,attachment-watermark,visual-regression}/`。源码 API 与使用示例见 [使用文档](../../usage.md#文字水印020)。本次不发布安装包，也不修改业务仓库的依赖。
