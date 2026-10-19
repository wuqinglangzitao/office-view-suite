# Office Viewer SDK 修订实施计划

本计划对应已批准并实施的 SDK 改造。`apps/*` 保留为可运行 demo，改造前提交 `ee6dbbe` 作为视觉回归基线。

## 阶段 0：锁定基线

1. 记录四个 demo 的现有 typecheck、production build 和样例打开结果。
2. 固定当前支持矩阵：Word `.doc/.docx/.docm`、Excel `.xlsx/.xlsm`、PPT `.pptx`、PDF `.pdf`。
3. 为每种格式保留至少一个覆盖图片、表格、字体、分页/懒加载的样例。
4. 在实现 SDK 前不改变现有 demo 的解析和渲染逻辑。

## 阶段 1：建立公共契约

1. 新增 `packages/viewer-core`，定义 `OfficeSource`、`OfficeOpenRequest`、`OfficeFormat`、`OfficeViewer`、`OfficeOpenResult`、错误码和事件类型；`open()` 统一接收 `{ bytes, name, format, signal }`。
2. 定义状态机、request id、`AbortSignal` 和 `destroy()` 语义。
3. 为 Worker 通信定义统一的 request/response envelope；格式 adapter 的文档模型保持私有。
4. 写契约测试，覆盖格式推断、取消、重复打开、错误码和销毁后的调用行为。

## 阶段 2：抽离格式 adapter

1. 将 Word、Excel、PPT 的解析 Worker 和 renderer 从 demo 入口拆到对应 `packages/viewer-*`。
2. 把现有 `render*Into` 改成 adapter 内部 renderer，保留现有样式和视觉行为。
3. 将 PDF 的解析、页面懒渲染、IntersectionObserver 和 canvas 管理拆成 `viewer-pdf` renderer。
4. 所有 adapter 返回统一的统计、警告和错误结构。

## 阶段 3：解决资产加载

1. 为每个 adapter 生成可发布的 Worker 和 WASM 资产清单。
2. 将 Word、Excel、PPT 的 Worker URL 改为构建产物可解析的相对资源。
3. 将 PDF classic Worker 改为接收资产 URL 配置，移除所有站点根路径绝对 URL。
4. 支持 `assets.baseUrl` 和 PDF 显式 URL 覆盖。
5. 增加非根路径、CDN、CORS 和 CSP smoke test。

## 阶段 4：实现 facade 和 demo 迁移

1. 新增 `packages/viewer`，实现 `createOfficeViewer()` 和按格式动态加载。
2. 统一 Shadow DOM、主题变量、ResizeObserver 和容器尺寸行为。
3. 将四个 demo 的文件选择、拖拽、状态 UI 保留在 `apps/*/src/app`，只通过 SDK API 打开文件。
4. 验证 demo 与第三方消费项目使用的是同一套 SDK 入口。

## 阶段 5：消费项目和发布包验证

1. 新增最小 vanilla/Vite fixture，使用发布后的 package 输出而不是 workspace 源码别名。
2. 运行两个 viewer 实例，验证并行解析和独立销毁。
3. 依次打开大文件、快速切换文件、取消解析、销毁宿主，再检查无旧响应和无未处理 Promise。
4. 检查发布包文件列表，确保没有 `rs/vendor`、`target`、`node_modules` 和 demo 样例。
5. 运行四个 demo 的 typecheck、build 和视觉回归。

第三方消费验收必须使用构建后的 package 输出（不引用 workspace 源码别名），并至少覆盖：

- 原生 ESM 和 Vite 两种入口；
- `/office-view/` 子路径部署；
- 两个实例并行打开、重复打开、取消和销毁；
- `assets.baseUrl` 与 PDF 显式资产 URL；
- 包内容不含 Rust vendor、`target`、`node_modules` 和 demo 样例。

## 阶段 6：发布准备

1. 为公共 API、错误码、浏览器边界、CSP、资产托管和隐私行为编写 README。
2. 配置 package `exports`、类型入口、运行时资产和许可证文件。
3. 采用 semver 发布；先发布 alpha 或内部包，完成真实第三方消费验证后再稳定发布。
4. 将 SDK 变更拆成可回滚提交：契约、adapter、资产、demo 迁移、消费验证。

## 阻断条件

- 如果无法在第三方 Vite fixture 中从发布包加载 Worker/WASM，不能进入 demo 迁移阶段。
- 如果 `destroy()` 后仍存在 Worker、Observer 或旧响应，不能进入发布阶段。
- 如果 PDF 仍依赖站点根路径资源，不能宣称支持子路径或 CDN 部署。
- 如果视觉回归显示现有 Word、Excel、PPT、PDF 行为变化，必须先修复 adapter 兼容性。

## 当前状态

2026-10-16 已完成阶段 0–5 及本地打包准备：

- 新增 core、四个适配器和 facade 六个包，Demo 仅通过 SDK 调用；移除旧重复渲染器/Worker。
- 构建输出独立 Worker/WASM、类型声明和 Rust 依赖许可证；独立消费项目只安装 `.tgz`，不使用源码别名。
- 公共 API 支持 bytes/name/format/signal、事件、clear/resize/destroy、多实例和旧会话隔离；自动格式识别和资源覆盖已实现。
- `pnpm build`、`pnpm typecheck`、37 项 Node 回归通过；浏览器验证覆盖四种格式、Vite/原生 ESM、子路径、跨域资源与生命周期。
- 6 个样本、45 页／工作表与旧渲染器截图一致，包括架构评审 PPT 全部 39 页。该验证比较迁移前后的渲染器，不代表与 Office 原生渲染完全一致。
- 接入和部署说明见 `docs/sdk.md`。按需格式子路径入口暂不提供，统一使用 facade。

未执行 npm 发布、Git 提交或推送。发布前仍需确定仓库自身许可证，并核验外部预编译 PDFium 的完整 Chromium 第三方声明；当前安装包包含 Rust 依赖声明及 PDFium 来源说明。Webpack/Rollup、Safari/Firefox 尚未完成实测。
