# PDFium 8046d 构建与许可核验记录

核验日期：2026-10-19。目标仅为 [8046d 发布包](https://github.com/paulocoutinhox/pdfium-lib/releases/tag/8046d)内 `release/node/pdfium.std.js` 与 `release/node/pdfium.std.wasm`。本次补齐许可及来源材料，未替换渲染引擎。

## 二进制与构建提交

发布包 `wasm.zip` 的 SHA-256 与 [GitHub release API](https://api.github.com/repos/paulocoutinhox/pdfium-lib/releases/tags/8046d) 返回的 asset `554203100` 的 digest 一致；两个解出文件与仓库文件逐字节相同。具体值见 [SOURCE.md](SOURCE.md) 和 [provenance.json](provenance.json)。发布包本身有 35 个文件，未附 LICENSE/NOTICE 文件，因此以下许可从对应源码收集。

[发布工作流 run 34435296619](https://github.com/paulocoutinhox/pdfium-lib/actions/runs/34435296619) 的日志打印了实际源码检出和工具链版本。以下摘录保留原始时间戳；不是根据目前分支头推测历史构建：

```text
Ubuntu	UNKNOWN STEP	2026-09-10T03:57:33.1780541Z HEAD is now at f2bed3c Merge pull request #183 from paulocoutinhox/support-arm64-linux-windows
Ubuntu	UNKNOWN STEP	2026-09-10T03:58:33.4532447Z emcc (Emscripten gcc/clang-like replacement + linker emulating GNU ld) 6.0.0 (afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3)
Ubuntu	UNKNOWN STEP	2026-09-10T04:00:49.6795989Z HEAD is now at f91ca5a Roll Zlib from c5cc9edf8992 to 285e94b8fa95 (4 revisions)
Ubuntu	UNKNOWN STEP	2026-09-10T04:00:49.7804561Z HEAD is now at 2cd94c0 [MiraclePtr] Enforce `raw_ptr` in `std::unordered_set`
Ubuntu	UNKNOWN STEP	2026-09-10T04:00:49.8425136Z HEAD is now at 640f254 Add LICENSE.md.chromium
Ubuntu	UNKNOWN STEP	2026-09-10T04:00:49.8792438Z HEAD is now at 89615a6 PA: Make SchedulerLoopQuarantineBranch use `enum class QuarantineTarget`
Ubuntu	UNKNOWN STEP	2026-09-10T04:00:51.5999209Z shared:INFO: (Emscripten: Running sanity checks)
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:21.2064871Z ports:INFO: retrieving port: zlib from https://github.com/madler/zlib/archive/refs/tags/v1.3.2.tar.gz
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:21.3680768Z ports:INFO: unpacking port: zlib
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:21.4236400Z cache:INFO: generating port: sysroot/lib/wasm32-emscripten/libz.a... (this will be cached in "/home/runner/work/pdfium-lib/pdfium-lib/emsdk/upstream/emscripten/cache/sysroot/lib/wasm32-emscripten/libz.a" for subsequent builds)
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:22.7325145Z system_libs:INFO: compiled 15 inputs in 1.31s
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:22.7919443Z cache:INFO:  - ok
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:22.7922740Z ports:INFO: retrieving port: libjpeg from https://storage.googleapis.com/webassembly/emscripten-ports/jpegsrc.v9f.tar.gz
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:22.8836554Z ports:INFO: unpacking port: libjpeg
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:22.9318081Z cache:INFO: generating port: sysroot/lib/wasm32-emscripten/libjpeg.a... (this will be cached in "/home/runner/work/pdfium-lib/pdfium-lib/emsdk/upstream/emscripten/cache/sysroot/lib/wasm32-emscripten/libjpeg.a" for subsequent builds)
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:28.1058359Z system_libs:INFO: compiled 59 inputs in 5.17s
Ubuntu	UNKNOWN STEP	2026-09-10T04:09:28.1676172Z cache:INFO:  - ok
```

依赖提交来自该 PDFium 提交的 [DEPS](https://pdfium.googlesource.com/pdfium/+/f91ca5a72358bb0b00b4da9481b21fe668157614/DEPS)，libjpeg-turbo 等额外 checkout 也与构建日志核对。所有许可文件的独立来源地址记录在 manifest 中。

## 构建配置与收集范围

配置来源为固定 builder 提交的 [modules/common.py](https://github.com/paulocoutinhox/pdfium-lib/blob/f2bed3c8bd49569a6b5c2a0df3a1ae0def3d8871/modules/common.py) 和 [modules/wasm.py](https://github.com/paulocoutinhox/pdfium-lib/blob/f2bed3c8bd49569a6b5c2a0df3a1ae0def3d8871/modules/wasm.py)。GN 关键参数为：

```text
is_debug=false
pdf_use_partition_alloc=false
target_cpu="wasm"
target_os="emscripten"
pdf_enable_v8=false
pdf_enable_xfa=false
treat_warnings_as_errors=false
is_component_build=false
symbol_level=0
pdf_is_complete_lib=true
is_clang=false
use_custom_libcxx=false
```

链接使用 `em++`，关键选项为 `-O2 -s USE_ZLIB=1 -s USE_LIBJPEG=1 -s FORCE_FILESYSTEM=1 -s ASSERTIONS=1 -s ALLOW_MEMORY_GROWTH=1 -s STANDALONE_WASM=1 -s WASM_ASYNC_COMPILATION=0 --no-entry`。builder 的 [extras/wasm/utils/custom.cpp](https://github.com/paulocoutinhox/pdfium-lib/blob/f2bed3c8bd49569a6b5c2a0df3a1ae0def3d8871/extras/wasm/utils/custom.cpp) 也参与编译，故保留 builder 的 MIT 和其引用的 PDFium 许可。

- 日志中的 Ninja 编译/归档输入包括 Abseil、AGG 2.3、FreeType、HarfBuzz、ICU、Little CMS、libjpeg-turbo、OpenJPEG、Chromium zlib；另有 Dragonbox、fast_float 头文件依赖。
- 16 个 `Foxit*` 字体源文件参与归档，保留 PDFium/Foxit 源码署名，以及两个 MM 字体字节数组内的 Adobe 署名。测试用 Noto 字体未列入生产输入。
- V8、XFA、Skia、PartitionAlloc、Brotli、BigInt、libtiff、libpng、Fontations 和 Rust 图像解码器在此配置中未参与生产编译。FreeType 自定义 `ftmodule.h` / `ftoption.h` 及 GN 清单还排除了 BDF、PCF、gzip 和 `ft-hb*` 实现；`fthash.c` 的独立 MIT 仍被收集。
- 源码补充声明涵盖 HarfBuzz 的 MIT/ISC/Microsoft USE 数据、移植的 FontTools 许可，以及 OpenJPEG 中不同作者的 BSD 声明。ICU 保留完整上游聚合 LICENSE；PDFium 根 LICENSE 也完整保留其 Apache 附文。
- `use_custom_libcxx=false` 使用 Emscripten 自带运行库。收集同一工具链中的 Emscripten/Node.js、musl、LLVM C/C++ runtime、dlmalloc 声明；补充 generic musl、libc++ Ryu 和 JavaScript runtime 的版权/许可注释。LLVM 各运行库与补充注释是保守集合，不作为逐符号链接清单。

Emscripten ports 与 PDFium 自身的 Chromium 分支库分开记录。日志确认取用了以下两个归档，下载后的 SHA-512 均与固定工具链配方一致：

| Port | 固定配方 | 归档 SHA-512 |
| --- | --- | --- |
| zlib 1.3.2 | [zlib.py](https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/tools/ports/zlib.py) | `16fea4df307a68cf0035858abe2fd550250618a97590e202037acd18a666f57afc10f8836cbbd472d54a0e76539d0e558cb26f059d53de52ff90634bbf4f47d4` |
| libjpeg 9f | [libjpeg.py](https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/tools/ports/libjpeg.py) | `7f733d79cf176c690dcf127352f9aa7ec48000455944f286faae606cdeada6f6865b4a3f9f01bda8947b5b1089bb3e52d2b56879b6e871279ec5cbd1829304dc` |

## 文本保留与验证边界

许可正文保留上游内容，仅归一行结束符及末尾空行。`SUPPLEMENTAL_SOURCE_NOTICES.md` 与 `Emscripten-SOURCE-NOTICES.md` 明确标记为逐段源码摘录，并附固定提交和文件位置。FreeType 采用 FTL，fast_float 与 Emscripten 采用 MIT 选项；本项目没有把所有第三方代码重新许可为一种协议。

编译日志证明的是构建输入；优化、静态链接可能移除部分对象，因此保守收集不等于声明每段代码都在最终二进制中。`pnpm verify:pdfium` 证明当前文件与记录的二进制、许可清单匹配，不自动证明新增依赖或其他版本的许可完整性。这份记录不覆盖用户上传的文档素材，也不替仓库所有者选择原创代码许可证。

## 更新与复核

1. 从指定 release 取得归档，核对 release asset digest、解出文件 SHA-256；准确记录标签、包内路径及发布日期。
2. 从对应发布工作流确认 builder、PDFium、Emscripten 的实际提交，检查 GN 参数、链接参数和第三方编译输入。可用 `gh run view 34435296619 --repo paulocoutinhox/pdfium-lib --log` 复查本版本日志；历史日志失效时仍保留以上摘录及固定源码链接。
3. 按该版本 DEPS、内嵌源码、工具链及启用 ports 更新许可正文、独立版权声明和产品文档署名。新版本不能直接沿用本清单，不能只刷新哈希来绕过复核。
4. 更新 `SOURCE.md`、`NOTICE.md`、本文及 `provenance.json`，然后运行 `pnpm verify:pdfium`、`pnpm build:sdk`、`pnpm test`、`pnpm test:packages`。测试包含对源材料、实际 SDK 输出和 npm tarball 中对应文件的核对。
