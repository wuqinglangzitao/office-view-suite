# PDFium WASM 第三方许可声明

本声明对应 `pdfium-lib 8046d` 的 `pdfium.std.js` / `pdfium.std.wasm`。二进制来源、SHA-256 和实际构建提交见 [SOURCE.md](SOURCE.md)，逐组件版本、许可文件哈希及固定来源见 [provenance.json](provenance.json)。本项目未修改这两个上游二进制。

## 应保留的署名

This software is based in part on the work of the Independent JPEG Group.

Portions of this software are copyright © 1996-2026 The FreeType Project (https://www.freetype.org). All rights reserved.

Original code copyright 2014 Foxit Software Inc. http://www.foxitsoftware.com

内嵌字体程序中还保留了 Adobe Systems Incorporated 的 1990–1994 年版权文字；原文及对应源码位置见 [补充声明](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md)。这份声明保留上游归属信息，不对字体另行授予许可证。

## 组件与许可文本

以下按源码和构建记录列出组件。部分工具链运行库及嵌套源码声明采用保守收集范围，不表示每个函数都留在最终 WASM 中。完整范围与排除依据见 [BUILD_EVIDENCE.md](BUILD_EVIDENCE.md)。协议摘要不替代所链接的许可正文。

| 组件 | 协议 / 许可说明 | 随附文本 |
| --- | --- | --- |
| PDFium 与内置 Foxit 字体 | BSD-3-Clause (root LICENSE also carries Apache-2.0 text for portions of the source tree) | [pdfium-LICENSE.txt](licenses/pdfium-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| Abseil | Apache-2.0 | [abseil-LICENSE.txt](licenses/abseil-LICENSE.txt) |
| Anti-Grain Geometry | MIT-style permissive license (preserve copying verbatim) | [agg23-LICENSE.txt](licenses/agg23-LICENSE.txt) |
| Dragonbox | Apache-2.0 WITH LLVM-exception OR BSL-1.0 | [dragonbox-LICENSE-Boost.txt](licenses/dragonbox-LICENSE-Boost.txt)、[dragonbox-LICENSE-Apache2-LLVM.txt](licenses/dragonbox-LICENSE-Apache2-LLVM.txt) |
| fast_float | MIT option | [fast-float-LICENSE-MIT.txt](licenses/fast-float-LICENSE-MIT.txt) |
| FreeType | FTL option; hash helpers also MIT | [freetype-FTL.txt](licenses/freetype-FTL.txt)、[freetype-LICENSE.txt](licenses/freetype-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| HarfBuzz | MIT-Modern-Variant (Old MIT), plus source-specific MIT/ISC notices | [harfbuzz-COPYING.txt](licenses/harfbuzz-COPYING.txt)、[harfbuzz-ms-use-COPYING.txt](licenses/harfbuzz-ms-use-COPYING.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| FontTools (HarfBuzz instancer-derived code) | MIT | [fonttools-LICENSE.txt](licenses/fonttools-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| ICU | Unicode-3.0 plus the third-party notices in LICENSE | [icu-LICENSE.txt](licenses/icu-LICENSE.txt) |
| Little CMS | MIT | [lcms-LICENSE.txt](licenses/lcms-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| libjpeg-turbo (Chromium fork) | IJG AND BSD-3-Clause (per README.chromium) | [libjpeg-turbo-LICENSE.md](licenses/libjpeg-turbo-LICENSE.md)、[libjpeg-turbo-README.ijg](licenses/libjpeg-turbo-README.ijg) |
| OpenJPEG | BSD-2-Clause | [openjpeg-LICENSE.txt](licenses/openjpeg-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| zlib (Chromium fork) | Zlib; Chromium CPU detection source also BSD-3-Clause | [zlib-chromium-LICENSE.txt](licenses/zlib-chromium-LICENSE.txt)、[SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md)、[chromium-LICENSE.txt](licenses/chromium-LICENSE.txt) |
| Chromium source modifications | BSD-3-Clause | [chromium-LICENSE.txt](licenses/chromium-LICENSE.txt) |
| PDFium 嵌套源码声明 | Source-specific BSD, MIT, ISC and retained font copyright | [SUPPLEMENTAL_SOURCE_NOTICES.md](licenses/SUPPLEMENTAL_SOURCE_NOTICES.md) |
| pdfium-lib 的 WASM helper | MIT | [pdfium-lib-LICENSE.txt](licenses/pdfium-lib-LICENSE.txt)、[pdfium-LICENSE.txt](licenses/pdfium-LICENSE.txt) |
| Emscripten JS / 系统运行时 | MIT option (upstream root also offers NCSA; Node.js-derived portions MIT) | [Emscripten-LICENSE.txt](licenses/Emscripten-LICENSE.txt)、[Emscripten-AUTHORS.txt](licenses/Emscripten-AUTHORS.txt)、[Emscripten-SOURCE-NOTICES.md](licenses/Emscripten-SOURCE-NOTICES.md) |
| musl libc | MIT and embedded source-specific permissive notices | [musl-COPYRIGHT.txt](licenses/musl-COPYRIGHT.txt)、[Emscripten-SOURCE-NOTICES.md](licenses/Emscripten-SOURCE-NOTICES.md) |
| LLVM C/C++ 运行库 | Apache-2.0 WITH LLVM-exception; legacy MIT/NCSA and Ryu BSL-1.0 notices retained | [compiler-rt-LICENSE.txt](licenses/compiler-rt-LICENSE.txt)、[libcxx-LICENSE.txt](licenses/libcxx-LICENSE.txt)、[libcxxabi-LICENSE.txt](licenses/libcxxabi-LICENSE.txt)、[libunwind-LICENSE.txt](licenses/libunwind-LICENSE.txt)、[llvm-libc-LICENSE.txt](licenses/llvm-libc-LICENSE.txt)、[Emscripten-SOURCE-NOTICES.md](licenses/Emscripten-SOURCE-NOTICES.md) |
| dlmalloc | Public-domain declaration (upstream references CC0-1.0) | [Emscripten-SOURCE-NOTICES.md](licenses/Emscripten-SOURCE-NOTICES.md) |
| zlib（Emscripten port） | Zlib | [zlib-emscripten-LICENSE.txt](licenses/zlib-emscripten-LICENSE.txt) |
| IJG libjpeg（Emscripten port） | IJG | [IJG-9f-README.txt](licenses/IJG-9f-README.txt) |

## 许可选择与分发要求

- FreeType 在本分发中使用 FTL 选项，保留上面的 FreeType 文档署名；其独立 MIT 代码另保留原文。`freetype-LICENSE.txt` 对另一 GPL 选项的描述不表示本分发选用了 GPL。
- fast_float 使用 MIT 选项；Emscripten 使用 MIT 选项并保留原始双许可文本；Dragonbox 保留上游双许可及 LLVM 例外全文。PDFium 根 `LICENSE` 的 BSD 与 Apache 文本均保留，不将其误写为所有 PDFium 代码可任选 Apache。
- 分发时保留本声明、`licenses/` 中的版权、许可及免责条款，包括适用的源码补充声明。不得未经授权使用上游作者或项目名为产品背书。若修改并再分发第三方源码，还应遵守相应许可证关于修改标记、源文件及通知保留的要求。
- 安装包中本组材料位于 `dist/licenses/pdfium/`，PDF 静态资源目录 `dist/assets/pdf/pdfium/` 也保留副本。部署时可从“关于 / 开源许可”页链接本声明及许可正文；不必在每个文档预览页重复显示。
- Rust 的 `pdfium-render` 及其他 Rust 依赖不在这份原生引擎清单中，其声明随适配器的 `dist/THIRD_PARTY_NOTICES.md` 和其他 `dist/licenses/` 子目录提供。本清单也不授予本项目原创 SDK 或用户文档、图片等素材的许可。
