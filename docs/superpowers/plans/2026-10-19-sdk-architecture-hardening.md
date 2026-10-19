# SDK 架构与资源优化

依据 2026-10-19 架构审查及用户“优化吧”的确认实施。保留只读预览定位、公共 API、现有解析器及各自 Cargo.lock，不新增依赖。

## 设计取舍

- 采用格式包内 `rs/` 归属 Rust 源码、vendor 和构建输出；不继续让 SDK 构建依赖 apps，也不为一次目录迁移新增独立引擎 npm 包或 Cargo workspace。
- PDFium 是上游预编译引擎，移入 `third_party/pdfium/` 并保留来源、许可及哈希验证；本项目 Rust glue 归属 PDF 格式包。
- SDK 构建校验 WASM 的源码、锁文件、工具链声明、构建配置及生成产物指纹。缺失或不匹配时明确要求重建，不默默发布陈旧二进制。`pnpm wasm` 负责构建并记录指纹。
- facade 到 adapter 使用一次性内部输入所有权交接，复用已复制及预检的数据；直接使用 adapter 仍复制和校验，不能由普通请求字段跳过校验。
- PDF 仅保留视口及预加载区域的位图，离屏释放像素、保留 CSS 占位，返回视口时重新绘制；过期异步响应不得恢复已回收位图。
- WASM 输出契约保持私有，使用真实引擎和样例验证关键结构及数据，不引入公开 DTO 或通用 schema 框架。

## 实施顺序与所有权

1. 主执行：迁移 Rust/PDFium；更新构建、许可、文档引用；增加构建指纹与回归测试；验证 PDF 部署无重复资源。
2. 输入通道执行：修改 viewer-core / viewer 及对应生命周期回归；主执行整合 PDF 输入接收点。
3. PDF 执行：修改 PDF renderer 的位图回收和对应回归；避免修改输入处理部分。
4. 契约测试执行：新增 Word/Excel/PPT 真实 WASM 输出契约测试；不修改引擎和公共类型。
5. 主执行整合、完整验证并独立审查，修复审查发现的问题。

## 验收与回归

- 修改前保留现有生命周期、取消、格式识别、buffer 所有权测试；对新增资源/所有权行为先补针对性回归，再改实现。
- 构建指纹测试覆盖源码/vendor/锁文件变化、缺失产物、产物篡改和正常构建；SDK 构建不读取 apps 中的引擎资源。
- 输入测试覆盖 facade 只复制及预检一次、调用者 buffer 未被 detach、adapter 直接调用仍校验、取消/替换/销毁不回归。
- PDF 测试覆盖离屏清像素、布局不变、重入重绘、离屏期间异步响应、resize 与 dispose。
- 真实 WASM 契约验证 Word 页面/文本、Excel sheets/cells/merges、PPT slides/shape 及样例关键值。
- 运行 `pnpm wasm`、Rust 单元测试、`pnpm build`、`pnpm typecheck`、`pnpm test`、`pnpm test:packages`、浏览器及视觉回归。项目没有独立 lint 配置，以类型检查、测试和 diff 静态检查验证。
- 独立代码与架构审查后，记录实际证据和未覆盖范围。浏览器验证限于当前 Chromium 环境；不宣称已解决完整解析峰值内存。

## 执行结果

六项审查问题均已处理：

- 四个 Rust crate（含 vendor 和 Cargo.lock）移入对应格式包；PDFium 移入 `third_party/pdfium/`。SDK 构建与许可收集不再读取 apps 中的引擎资源。源码迁移保持内容不变。
- `build-wasm.mjs` 在 `--locked` 构建成功后记录指纹；SDK 准备资源和收集许可时均校验。覆盖源码、vendor、锁文件、仓库内各级 `.cargo`、工具链声明、构建脚本及 JS/类型/WASM 产物。独立审查发现的 Cargo 祖先配置漏项已修复。
- facade 使用 WeakMap 登记的请求对象进行一次性输入交接；公共打开路径回归证明仅复制一次、预检一次，普通 adapter 调用仍复制并校验，调用者 buffer 保持有效。
- PDF 离开可见/预加载区域时清空像素；重入时重绘；异步迟到响应、resize、dispose 均有回归覆盖。
- PDF demo 构建目录原先两套相同的 PDFium/glue WASM 现在各保留一套，减少重复 WASM **7,151,183 字节**。最终目录中的五个 WASM 哈希各不相同；其余格式引擎来自统一 viewer 的既有依赖。
- 增加真实 Word/Excel/PPT WASM 输出与当前 TS 模型的 `satisfies` 检查，以及页面单位、样式、坐标、合并和文本语义断言；故意破坏字段及嵌套类型的测试确认契约漂移会失败。

验证记录（2026-10-19）：

- `pnpm wasm`、`pnpm build`：通过；实际 SDK 资源准备命令也验证了构建配置变化后拒绝旧 WASM、提示重建。
- `pnpm typecheck`：11 个项目通过；`pnpm test`：111/111 通过，其中构建指纹 18 项。
- `pnpm test:rust`：Word 2、Excel 3、PPT 17，共 22 项通过。
- `pnpm test:packages`：六个 tarball 在独立消费者安装、类型检查、构建及资源校验通过。最终重新构建后的 545 个发布文件与已验证消费者逐文件一致。
- `pnpm test:browser`：20 个场景通过，浏览器错误和失败请求为空。新增 PDF 真实滚动场景验证仅保留可见页、布局不变、返回后的像素完全一致、clear 后像素全部释放。
- `pnpm test:visual`：6 份样例、45 页/工作表与 `ee6dbbe` 基线逐像素一致，9 项 Word 缩放回归通过；已查看 PDF 截图及 Excel 基线/当前截图。
- `pnpm test:excel`：9 个 DTO 场景、4 个 XLSX、7 项交互通过。
- 独立代码审查：APPROVE；独立架构审查：CLEAR；脚本语法及 `git diff --check` 通过。

保留边界：没有 IntersectionObserver 时 PDF 仍全量绘制；位图回收不约束引擎内部的整本解析内存；构建指纹不等于隔离全部环境的跨平台逐字节复现；样例契约测试不覆盖所有文档特性；Safari/Firefox 未实测。Rust 构建中原有的弃用 API 警告未在本轮改写。
