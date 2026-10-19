# 预览优化计划审查记录

日期：2026-10-19。

对象：`excel-viewer-optimization.md`，已同步只读预览边界的版本。

结论：需要修订后再实施（REQUEST CHANGES）；架构审查状态为 WATCH。代码事实与架构取舍由两个独立审查角色核对。本次只检查计划和相关源码，未运行性能测试，以下不代表已有性能测量结果。

## 已同步的范围结论

- 明确只读预览定位，区分 Excel 专项优化与共享 SDK 能力。
- P2 只允许声明实际实现的能力。
- P6 收缩为缩放、适应容器、工作表切换和滚动位置保持；文本搜索和页码/幻灯片导航为可选增强。
- 单元格/范围定位、选区、编辑、公式交互、打印和导出移出本轮范围。

## 待修订问题

### R1 — 高：P4 缺少主入口格式路由的修改范围

计划位置：`excel-viewer-optimization.md:82`、`:88`。

当前 `packages/viewer/src/index.ts:190` 优先使用文件名或显式格式，`:192` 已据此选择 adapter。仅修改 core 和 Worker，无法保证 XLSX 内容误命名为 `.pdf` 后仍进入 Excel adapter。

建议：将 facade 格式选择纳入 P4；明确内容识别、扩展名、显式 `format` 三者的优先级及冲突行为。验收分别覆盖受支持但错误的扩展名、未知扩展名、显式格式冲突。

### R2 — 中：P1 未定义虚拟化后的尺寸与跨视口对象处理

计划位置：`excel-viewer-optimization.md:43`、`:44`。

当前 `packages/viewer-excel/src/core/render.ts:138` 使用 `max-content` 列宽，`:200` 使用自动行高；`:143` 开始把合并范围逐格展开。只挂载可见内容后，屏外长文本可能影响列宽和行高，合并主单元格可能在视口外，巨大合并范围仍可能在 DOM 创建前消耗大量内存。

建议：先定义稳定尺寸计算或测量缓存策略；跨视口合并与图片/形状按相交区域处理，避免按合并面积展开覆盖集合。补充屏外长文本、换行、跨视口合并、冻结交叉区、巨大合并范围用例。将懒挂载与虚拟化分阶段交付，明确工作表缓存和释放策略。

### R3 — 中：P4 对轻量预检作出了过度承诺

计划位置：`excel-viewer-optimization.md:89`。

`packages/viewer-core/src/index.ts:142` 目前检查 PDF 标识，`:163` 遍历 ZIP 中央目录。有效签名不代表内容有效；PDF 对象流、引用或压缩数据损坏仍需解析引擎判断，不能保证所有无效 PDF 都在 WASM 启动前失败。

建议：预检只拒绝可确定的签名、容器边界和必要条目错误；深层错误由引擎报告 `PARSE_FAILED`。分别建立预检拒绝和解析阶段拒绝的样例，避免重复实现完整解析器。

### R4 — 中：P0/P1 未覆盖现有视觉测试的全量 DOM 假设

计划位置：`excel-viewer-optimization.md:34`、`:38`、`:132`。

`scripts/visual-sdk-regression.mjs:97` 在切换工作表前检查新旧 `.sheet` 数量相等，随后按索引截图。正确的懒挂载也可能被该测试判失败。该脚本的旧基准固定于 `:16`，且新旧展示共享当前解析后的 DTO（`:54`），不能同时证明本轮性能收益和解析兼容性。

建议：将脚本调整纳入 P0/P1，按文档中的工作表名称逐次激活、比较可见视口，不依赖全部工作表常驻 DOM。保留已有 pre-SDK 回归用途，另存本轮优化前基线与独立解析期望值。

### R5 — 中：P2 的优先级与“只注册即可新增格式”验收偏大

计划位置：`excel-viewer-optimization.md:54`、`:62`。

新增格式还涉及 `packages/viewer-core/src/index.ts:1` 的类型、`:131` 的白名单和识别规则，以及 `packages/viewer-excel/src/index.ts:15` 等 adapter 格式声明。替换 facade 分支不能独自完成新增格式支持。

建议：降为中优先级，先限定为内部静态映射整理；验收改为“为已识别格式添加 adapter 时，facade 主流程无需增加分支”。新增格式仍同步类型、识别和 adapter 验证；能力查询随实际操作落地，不作为 Excel 性能优化的前置条件。

### R6 — 中：P3 应复用已有资源机制，明确 manifest 的消费方

计划位置：`excel-viewer-optimization.md:67`、`:71`、`:77`。

已有共用资源输出配置 `scripts/sdk-vite-config.ts:3`、资源准备 `scripts/prepare-sdk-assets.mjs:5`、子路径和跨域 baseUrl 测试 `scripts/browser-sdk-smoke.mjs:23`、`:78`、许可证收集 `scripts/collect-sdk-notices.mjs:14`。默认资源使用 Vite 可识别的静态 URL（`packages/viewer-excel/src/index.ts:6`）。

建议：降为中优先级，在既有构建链上生成部署资源索引，明确由复制或打包校验脚本消费；保留 URL 覆盖优先级与 `workerFactory` 契约。构建阶段检查文件存在和清单完整性；HTTP MIME/CORS 通过受控部署测试验证，不能靠静态构建保证任意宿主部署正确。

### R7 — 中：P5 应先固定比较层次，避免提前引入备用解析器

计划位置：`excel-viewer-optimization.md:94`、`:96`、`:102`。

现有链路包括 `packages/viewer-excel/src/worker/worker.ts:23` 的 WASM 转换和传输、`packages/viewer-excel/src/core/types.ts:3` 的 ExcelDoc、`packages/viewer-excel/src/index.ts:36` 的 renderer。如果同时更换解析器和渲染器，无法把改善归因到解析、DTO 转换或展示层。

建议：本阶段只做现有 `xlsx/xlsm` 范围的独立 benchmark，记录原始解析信息与映射到统一 ExcelDoc 后的结果，使用相同 renderer；分开统计初始化、解析、转换/传输与首屏时间。生产 fallback 是否接入，依据差异归因和收益报告另行决定。

## 建议实施次序

先补齐 P0 基线和验收方法，再交付工作表懒挂载及虚拟化；P4 按明确的预检边界推进。P2/P3 降为中优先级的增量整理，P6 保持只读基础操作范围；P5 由真实兼容性缺口触发评估。

2026-10-19 更新：用户已授权更新文档、提交并开始实施。上述 R1–R7 已合入主计划，原计划行号仅记录审查时的位置；对应关系见主计划“审查意见处理”。本记录保留历史发现，不表示当前仍未修订。
