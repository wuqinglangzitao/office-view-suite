# Office View Suite

在浏览器中预览 Word、Excel、PowerPoint 和 PDF，提供可直接嵌入第三方应用的 TypeScript SDK，以及使用同一 SDK 的演示应用。

文件由 Worker 与 Rust/WASM 在浏览器内解析，SDK 不上传文档内容。通过原生 DOM、Shadow DOM 和 Canvas 渲染，无需 iframe；支持多实例、取消加载和组件卸载时释放资源。

当前提供源码及可本地打包的 ESM 包，**尚未发布到 npm**。项目自身的许可证尚未确定，使用和分发前请查看[许可状态](#项目许可状态)。

[快速开始](#快速开始) · [SDK 接入](#sdk-接入) · [开发与验证](#开发与验证) · [解析依赖与许可证](#解析依赖与许可证)

## 支持格式

| 文档 | 扩展名 | 预览方式 |
| --- | --- | --- |
| Word | `.doc`、`.docx`、`.docm` | DOM 渲染，渐进分页 |
| Excel | `.xlsx`、`.xlsm` | 当前工作表懒挂载，大表可见区域渲染 |
| PowerPoint | `.pptx` | DOM / SVG 渲染，按容器宽度适配 |
| PDF | `.pdf` | PDFium + Canvas，按可见页面懒渲染 |

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

这些开发命令会先构建 SDK。修改 SDK 源码后，执行 `pnpm build:sdk` 刷新演示应用使用的产物，再刷新页面。

## SDK 接入

公共入口为 `@office-view-suite/viewer`。当前可在工作区内引用；跨项目接入需要本地构建六个包并使用 `.tgz` 安装，具体方法见 [SDK 构建与安装](docs/sdk.md#构建与安装)。

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

Vite 可处理包内 Worker/WASM 资源；自行部署静态资源或使用 CDN 时，需要保留资源目录并配置 URL、CORS 与 CSP。完整 API、取消操作、原生 ESM 及部署示例见 [SDK 接入文档](docs/sdk.md)。

## 仓库结构

使用 pnpm workspaces 管理 SDK、演示应用和验证项目：

```text
packages/
  viewer/                    # 公共入口，按格式动态加载适配器
  viewer-core/               # 公共类型、格式识别与生命周期基础能力
  viewer-{word,excel,ppt,pdf}/ # 各格式适配器、渲染器与 Worker
apps/*-view/                 # 演示 UI、rs/ Rust 解析器、public/ 静态资源
fixtures/vanilla-vite/       # 从构建产物接入 SDK 的多实例示例
tests/                      # SDK 回归测试
scripts/                    # 构建、许可收集和安装包验证
docs/sdk.md                 # 接入与部署说明
```

SDK 运行时代码位于 `packages/`；文件选择等演示交互位于 `apps/`。贡献约定见 [AGENTS.md](AGENTS.md)。

## 开发与验证

| 命令 | 用途 |
| --- | --- |
| `pnpm build` | 按依赖顺序构建 SDK、演示应用和接入示例 |
| `pnpm build:sdk` | 仅构建六个 SDK 包 |
| `pnpm typecheck` | 检查整个工作区的 TypeScript 类型 |
| `pnpm test` | 运行 Node.js SDK 回归测试 |
| `pnpm test:packages` | 打包、核对许可材料，在独立临时项目安装并构建 |
| `pnpm test:excel` | Excel 大表性能、工作表切换与布局回归（需要 Playwright） |
| `pnpm verify:pdfium` | 离线核对 PDFium 二进制、许可文本哈希和组件引用 |
| `pnpm wasm` | 重建所有 Rust/WASM 产物 |

验证前先完成构建；类型检查和部分测试依赖 `dist/`，不要与构建并行执行：

```sh
pnpm build
pnpm typecheck
pnpm test
pnpm test:packages
```

可按格式执行 `build:<格式>`、`typecheck:<格式>`、`wasm:<格式>`，例如 `pnpm wasm:ppt`，格式为 `word`、`excel`、`ppt`、`pdf`。浏览器与视觉回归另提供 `pnpm test:browser`、`pnpm test:visual`，需要先运行安装包验证，并准备 Chrome 和已安装的 Playwright 模块；配置见 [验证说明](docs/sdk.md#验证和边界)。

## 解析依赖与许可证

以下列出主要解析依赖及当前锁定版本；各适配器的第三方声明随 SDK 安装包提供。

| 用途 | 依赖 | 版本 | 许可证 |
| --- | --- | --- | --- |
| Word 解析 | [rwml](https://github.com/HyunjoJung/rwml) | `0.1.4` | [MIT](apps/word-view/rs/vendor/rwml/LICENSE) |
| Excel 解析 | [umya-spreadsheet](https://github.com/MathNya/umya-spreadsheet) | `3.1.1` | [MIT](apps/excel-view/rs/vendor/umya-spreadsheet/LICENSE) |
| PPT XML 工具 | [ppt-rs](https://github.com/yingkitw/ppt-rs) | `0.2.36` | [Apache-2.0](apps/ppt-view/rs/vendor/ppt-rs/LICENSE) |
| PDF Rust 绑定 | [pdfium-render](https://github.com/ajrcarey/pdfium-render) | `0.9.4` | [MIT OR Apache-2.0](https://docs.rs/crate/pdfium-render/0.9.4) |
| PDF 渲染引擎 | [PDFium](https://pdfium.googlesource.com/pdfium/) | `pdfium-lib 8046d` / `chromium/8046` | 主许可 BSD-3-Clause，另含 Apache-2.0 及[各组件许可](apps/pdf-view/public/pdfium/NOTICE.md) |

Word、Excel、PPT 使用 Rust/WASM 解析，再由本项目渲染器展示。PPT 基于 `ppt-rs` 的 XML 工具及 `zip` 实现自研 OOXML 解析；前三项库的内置源码位于 `apps/*-view/rs/vendor/`，包含本地修复并保留原许可证。`MIT OR Apache-2.0` 表示可选择其中一种许可。

### 第三方声明如何分发

各适配器包包含 `dist/THIRD_PARTY_NOTICES.md` 和 `dist/licenses/`。PDFium 材料还附在 `dist/assets/pdf/pdfium/`，方便随完整静态资源目录部署；若构建工具只复制代码引用的资源，需要另外部署许可文档。

当前 PDFium 材料覆盖对应构建的解析引擎、内嵌组件及字体、Emscripten 运行库。源码提交、文件哈希和收集范围可从以下文档复核：

- [第三方许可与必要署名](apps/pdf-view/public/pdfium/NOTICE.md)
- [二进制来源与版本](apps/pdf-view/public/pdfium/SOURCE.md)
- [构建证据与升级流程](apps/pdf-view/public/pdfium/BUILD_EVIDENCE.md)
- [SDK 许可材料部署方式](docs/sdk.md#第三方许可随包部署)

更换 PDFium 二进制时须同步复核许可材料，不能只刷新哈希。样本文档、图片等业务素材的授权需单独确认。

### 项目许可状态

仓库尚无根目录 `LICENSE`，根包及 SDK 包也未声明 `license` 字段。公开仓库和第三方依赖的宽松许可证，不等于本项目原创代码已经获得明确的开源使用或商业分发授权。

确认原创代码版权归属后，可考虑 **Apache-2.0（建议）**、**MIT** 或 **MIT OR Apache-2.0**；这些方案尚未采用。Apache-2.0 包含明确的专利授权条款；选择 MIT 或双许可证也不能改变随包第三方组件的原许可义务。正式采用时应补充根目录许可证及各发布包的许可元数据，并保留第三方版权、许可、适用的 NOTICE 和修改声明。

## 当前验证范围

已验证 Chrome 下的 Vite 生产构建、原生 ESM、多实例、取消/替换加载、资源路径覆盖及销毁流程。SDK 需要支持 Worker、WebAssembly、Shadow DOM、ResizeObserver 的现代浏览器；PDF 懒渲染还使用 IntersectionObserver。

SSR、UMD 不在当前支持范围；Webpack/Rollup 独立接入及 Safari/Firefox 尚未完成实测。视觉回归用于验证 SDK 迁移前后的渲染一致性，不代表与 Office 原软件逐像素一致。
