# Office View Suite

在浏览器中预览 Word、Excel、PowerPoint 和 PDF，提供可直接嵌入第三方应用的 TypeScript SDK，以及使用同一 SDK 的演示应用。

文件由 Worker 与 Rust/WASM 在浏览器内解析，SDK 不上传文档内容。通过原生 DOM、Shadow DOM 和 Canvas 渲染，无需 iframe；支持多实例、取消加载和组件卸载时释放资源。

当前提供源码及可本地打包的 ESM 包，**尚未发布到 npm**。项目原创代码采用 [Apache-2.0](LICENSE)，允许商业使用，分发要求见[项目许可证](#项目许可证)。

[快速开始](#快速开始) · [SDK 接入](#sdk-接入) · [开发与验证](#开发与验证) · [已知预览差异](docs/known-preview-differences.md) · [解析依赖与许可证](#解析依赖与许可证)

## 支持格式

| 文档 | 扩展名 | 预览方式 |
| --- | --- | --- |
| Word | `.doc`、`.docx`、`.docm` | DOM 渲染，渐进分页 |
| Excel | `.xlsx`、`.xlsm` | 当前工作表懒挂载，大表可见区域渲染 |
| PowerPoint | `.pptx` | DOM / SVG 渲染，按容器宽度适配 |
| PDF | `.pdf` | PDFium + Canvas，按可见页面懒渲染，离屏回收位图 |

不支持旧版 `.xls`、`.ppt`。预览效果受文档特性、可用字体和解析器支持范围影响，不保证与 Microsoft Office 像素级一致。

## 快速开始

### 环境要求

- Node.js **22.15+**、pnpm **7.33.7**。
- Rust **1.89.0**，仓库通过 [rust-toolchain.toml](rust-toolchain.toml) 固定工具链及 WASM 目标。
- `wasm-pack`：编译 Rust/WASM；SDK 构建还会使用 Cargo 收集依赖许可。

### 启动演示

在仓库根目录执行：

```sh
pnpm install
pnpm wasm       # 首次检出或修改 Rust 解析器后执行
pnpm dev:sdk    # 构建 SDK 并启动多实例接入示例
```

打开终端中 Vite 输出的地址。也可以启动单格式演示：

| 演示 | 命令 |
| --- | --- |
| Word | `pnpm dev:word` |
| Excel | `pnpm dev:excel` |
| PowerPoint | `pnpm dev:ppt` |
| PDF | `pnpm dev:pdf` |
| Vue 2.7 接入示例 | `pnpm dev:vue2` |

这些开发命令会先构建 SDK。修改 TypeScript 源码后，执行 `pnpm build:sdk` 刷新演示应用使用的产物，再刷新页面。修改 Rust、vendor、Cargo.lock、工具链声明或 WASM 构建脚本后，应先运行 `pnpm wasm`；SDK 构建会拒绝缺失、过期或被修改的 WASM 产物，并提示对应的重建命令。

## SDK 接入

公共入口为 `@office-view-suite/viewer`，可从前端私服安装；其余五个包会自动安装。作用域配置、离线 `.tgz` 交付和源码构建见 [SDK 构建与安装](docs/sdk.md#构建与安装)。

```sh
pnpm add @office-view-suite/viewer@0.1.2 --registry=http://npm.dev.e6gpshk.com:38085/repository/npm-group/
```

在页面中准备文件选择框和具有可测量宽度的容器：

```html
<label>
  选择文档
  <input id="office-file" type="file" accept=".doc,.docx,.docm,.xlsx,.xlsm,.pptx,.pdf" />
</label>
<div id="office-preview" style="width: 100%"></div>
```

在 DOM 挂载后执行以下 TypeScript：

```ts
import { createOfficeViewer } from '@office-view-suite/viewer';

const container = document.querySelector<HTMLElement>('#office-preview')!;
const input = document.querySelector<HTMLInputElement>('#office-file')!;
const viewer = createOfficeViewer(container);

// 接收后台渲染错误，例如 PDF 后续页面加载失败。
viewer.on(event => {
  if (event.type === 'error') console.error(event.error);
});

input.addEventListener('change', () => {
  const file = input.files?.[0];
  if (file) {
    viewer.open({ bytes: file, name: file.name }).catch(console.error);
  }
});
```

`bytes` 接受 `File` / `Blob`、`ArrayBuffer` 或 `Uint8Array`。同一实例再次 `open()` 会取消前一次加载；`clear()` 清空预览，`resize()` 可在隐藏容器重新显示后调用。React / Vue 等框架在挂载后创建实例，在卸载时调用 `viewer.destroy()`，并清理宿主自己注册的监听。

Vite 生产构建可处理包内 Worker/WASM 资源；开发模式需将六个 SDK 包加入 `optimizeDeps.exclude`，保留资源相对路径。已实测的组件和配置见 [Vue 2 + Vite 接入](docs/vue2.md)。自行部署静态资源或使用 CDN 时，需要保留资源目录并配置 URL、CORS 与 CSP。完整 API、取消操作、原生 ESM 及部署示例见 [SDK 接入文档](docs/sdk.md)。

## 仓库结构

使用 pnpm workspaces 管理 SDK、演示应用和验证项目：

```text
packages/
  viewer/                    # 公共入口，按格式动态加载适配器
  viewer-core/               # 公共类型、格式识别与生命周期基础能力
  viewer-{word,excel,ppt,pdf}/ # 各格式适配器、渲染器与 Worker
    rs/                      # Rust 源码、vendor、Cargo.lock
    src/wasm/                # wasm-pack 生成产物与构建指纹（不入库）
third_party/pdfium/          # 上游预编译 PDFium、来源及许可材料
apps/*-view/                 # 演示 UI 和 public/samples 样例
fixtures/vanilla-vite/       # 从构建产物接入 SDK 的多实例示例
fixtures/vue2-vite/          # Vue 2.7 Options API 组件与 Vite 接入验证
tests/                      # SDK 回归测试
scripts/                    # 构建、许可收集和安装包验证
docs/sdk.md                 # 接入与部署说明
```

SDK 源码和引擎构建归属 `packages/`，构建不依赖演示应用目录；文件选择等演示交互位于 `apps/`。应用最终 `dist/assets/` 中的 WASM 是正常部署资源。PDFium 本体由上游编译，`pnpm wasm:pdf` 只构建本项目 Rust glue。贡献约定见 [AGENTS.md](AGENTS.md)。

## 开发与验证

| 命令 | 用途 |
| --- | --- |
| `pnpm build` | 按依赖顺序构建 SDK、演示应用和接入示例 |
| `pnpm build:sdk` | 仅构建六个 SDK 包 |
| `pnpm typecheck` | 检查整个工作区的 TypeScript 类型 |
| `pnpm test` | 运行 Node.js SDK 回归测试 |
| `pnpm test:rust` | 运行 Word、Excel、PPT crate 的原生单元测试 |
| `pnpm test:packages` | 打包、核对许可材料，在独立临时项目安装并构建 |
| `pnpm test:vue2` | 独立安装包的 Vue 2 开发/生产浏览器验证（需先运行 test:packages） |
| `pnpm test:excel` | Excel 大表性能、工作表切换与布局回归（需要 Playwright） |
| `pnpm test:word:display` | Word 展示数据、字段与布局回归（需要 Playwright） |
| `pnpm test:word:charts` | Word SVG 图表、数值与降级数据展示回归（需要 Playwright） |
| `pnpm test:ppt:display` | PPT 渐变、字号、表格、层级、变换与裁剪回归（需要 Playwright） |
| `pnpm verify:pdfium` | 离线核对 PDFium 二进制、许可文本哈希和组件引用 |
| `pnpm wasm` | 以锁定依赖构建所有 Rust/WASM，并记录输入与产物指纹 |

验证前先完成构建；类型检查和部分测试依赖 `dist/`，不要与构建并行执行：

```sh
pnpm build
pnpm typecheck
pnpm test
pnpm test:rust
pnpm test:packages
```

可按格式执行 `build:<格式>`、`typecheck:<格式>`、`wasm:<格式>`，例如 `pnpm wasm:ppt`，格式为 `word`、`excel`、`ppt`、`pdf`。浏览器与视觉回归另提供 `pnpm test:browser`、`pnpm test:visual`，需要先运行安装包验证，并准备 Chrome 和已安装的 Playwright 模块；配置见 [验证说明](docs/sdk.md#验证和边界)。

## 解析依赖与许可证

以下列出主要解析依赖及当前锁定版本；各适配器的第三方声明随 SDK 安装包提供。

| 用途 | 依赖 | 版本 | 许可证 |
| --- | --- | --- | --- |
| Word 解析 | [rwml](https://github.com/HyunjoJung/rwml) | `0.1.4` | [MIT](packages/viewer-word/rs/vendor/rwml/LICENSE) |
| Excel 解析 | [umya-spreadsheet](https://github.com/MathNya/umya-spreadsheet) | `3.1.1` | [MIT](packages/viewer-excel/rs/vendor/umya-spreadsheet/LICENSE) |
| PPT XML 工具 | [ppt-rs](https://github.com/yingkitw/ppt-rs) | `0.2.36` | [Apache-2.0](packages/viewer-ppt/rs/vendor/ppt-rs/LICENSE) |
| PDF Rust 绑定 | [pdfium-render](https://github.com/ajrcarey/pdfium-render) | `0.9.4` | [MIT OR Apache-2.0](https://docs.rs/crate/pdfium-render/0.9.4) |
| PDF 渲染引擎 | [PDFium](https://pdfium.googlesource.com/pdfium/) | `pdfium-lib 8046d` / `chromium/8046` | 主许可 BSD-3-Clause，另含 Apache-2.0 及[各组件许可](third_party/pdfium/NOTICE.md) |

Word、Excel、PPT 使用 Rust/WASM 解析，再由本项目渲染器展示。PPT 基于 `ppt-rs` 的 XML 工具及 `zip` 实现自研 OOXML 解析；前三项库的内置源码位于 `packages/viewer-*/rs/vendor/`，包含本地修复并保留原许可证。`MIT OR Apache-2.0` 表示可选择其中一种许可。

### 第三方声明如何分发

各适配器包包含 `dist/THIRD_PARTY_NOTICES.md` 和 `dist/licenses/`。PDFium 材料还附在 `dist/assets/pdf/pdfium/`，方便随完整静态资源目录部署；若构建工具只复制代码引用的资源，需要另外部署许可文档。

当前 PDFium 材料覆盖对应构建的解析引擎、内嵌组件及字体、Emscripten 运行库。源码提交、文件哈希和收集范围可从以下文档复核：

- [第三方许可与必要署名](third_party/pdfium/NOTICE.md)
- [二进制来源与版本](third_party/pdfium/SOURCE.md)
- [构建证据与升级流程](third_party/pdfium/BUILD_EVIDENCE.md)
- [SDK 许可材料部署方式](docs/sdk.md#第三方许可随包部署)

更换 PDFium 二进制时须同步复核许可材料，不能只刷新哈希。样本文档、图片等业务素材的授权需单独确认。

### 项目许可证

项目原创代码采用 **Apache License 2.0**，完整条款见 [LICENSE](LICENSE)。根包、SDK 包、演示应用及本项目 Rust crate 均声明 `Apache-2.0`；每个 SDK 安装包包含 `dist/LICENSE`。

允许商业使用、修改和分发，也可集成到闭源应用，无须公开业务应用源码。分发时应提供许可证副本，保留适用的版权、署名和 NOTICE；分发修改后的文件时应明确标注改动。专利授权和其他条件以许可证正文为准。

第三方依赖、内置源码及 PDFium 组件继续遵循各自许可证，具体材料见上文；业务样本文档、图片等素材不因本项目采用 Apache-2.0 而自动获得授权。

## 当前验证范围

已验证 Chrome 下的 Vite 生产构建、原生 ESM、多实例、取消/替换加载、资源路径覆盖及销毁流程。SDK 需要支持 Worker、WebAssembly、Shadow DOM、ResizeObserver 的现代浏览器；PDF 懒渲染还使用 IntersectionObserver。

Vue 2.7.16 + Vite 6.4.4 已通过开发/生产及两种资源部署方式的 44 项接入检查；包括四格式渲染、文件切换、组件卸载和 Worker 释放。可复制组件与验证步骤见 [Vue 2 接入文档](docs/vue2.md)。

SSR、UMD 不在当前支持范围；Webpack/Rollup 独立接入及 Safari/Firefox 尚未完成实测。视觉回归用于验证 SDK 迁移前后的渲染一致性，不代表与 Office 原软件逐像素一致。
