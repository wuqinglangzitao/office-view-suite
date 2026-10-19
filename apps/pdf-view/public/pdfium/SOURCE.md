# PDFium WASM 来源

本目录的 `pdfium.std.js` 和 `pdfium.std.wasm` 原样取自以下发布包，必须配对使用。本项目未修改这两个文件。

| 项目 | 固定来源 |
| --- | --- |
| 二进制发布 | [paulocoutinhox/pdfium-lib 8046d](https://github.com/paulocoutinhox/pdfium-lib/releases/tag/8046d)，2026-09-10 |
| 下载文件 | [wasm.zip](https://github.com/paulocoutinhox/pdfium-lib/releases/download/8046d/wasm.zip)，13,414,692 字节 |
| 包内路径 | `release/node/pdfium.std.js`、`release/node/pdfium.std.wasm` |
| 构建仓库提交 | `f2bed3c8bd49569a6b5c2a0df3a1ae0def3d8871` |
| PDFium 源码提交 | `f91ca5a72358bb0b00b4da9481b21fe668157614`，分支 `chromium/8046` |
| Emscripten | `6.0.0`，提交 `afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3` |
| 构建记录 | [GitHub Actions run 34435296619](https://github.com/paulocoutinhox/pdfium-lib/actions/runs/34435296619) |

原来源记录中的 `chromium/8046d` 混合了发布标签与源码分支；准确发布标签为 `8046d`。

## SHA-256 校验

2026-10-19 重新下载发布包，归档哈希与 GitHub release asset 公布值一致；解出的两个文件与本目录文件逐字节一致。

| 文件 | SHA-256 |
| --- | --- |
| `wasm.zip` | `062d10055de90a01120acf307ce16e0022021a32203f4aeae4877750f9a08427` |
| `pdfium.std.js` | `30d0b02a8d828627838a0d6b62a2ae960dc1d6fc24081ca426afaaae0f37c8c9` |
| `pdfium.std.wasm` | `223e608c0980885d8fb31638f1533b6c62dba1803b9595ba9fd34879064ceee0` |

## 许可与部署

许可清单、署名和分发要求见 [NOTICE.md](NOTICE.md)，原文保存在 [licenses/](licenses/)。PDFium 主许可为 BSD-3-Clause；其原始 `LICENSE` 还包含 Apache-2.0 文本，其他组件分别适用其许可证，不能将整个 WASM 简化为只受 BSD-3-Clause 约束。

[BUILD_EVIDENCE.md](BUILD_EVIDENCE.md) 记录构建来源、依赖选择和复核方法；[provenance.json](provenance.json) 保存二进制、许可文件哈希及每份文本的上游来源。在仓库根目录运行 `pnpm verify:pdfium` 可离线验证。升级二进制时须同步复核并更新这组材料。

此构建要求调用方预取 WASM，并通过 `wasmBinary` 传入；不支持自动同步获取。部署时将本目录许可文档和 `licenses/` 与 JS/WASM 一并保留。
