# @office-view-suite/viewer

浏览器端 Word、Excel、PowerPoint 和 PDF 预览 SDK。原生 DOM、Shadow DOM、Worker、WASM，无 iframe。

当前版本 **0.2.0**，已发布到 [npm](https://www.npmjs.com/package/@office-view-suite/viewer)。

```sh
pnpm add @office-view-suite/viewer@0.2.0
# 或使用 npm
npm install @office-view-suite/viewer@0.2.0
```

```ts
import { createOfficeViewer } from '@office-view-suite/viewer';

const viewer = createOfficeViewer(document.querySelector<HTMLElement>('#preview')!);
await viewer.open({ bytes: file, name: file.name });
// 组件卸载时释放 Worker、观察器和预览 DOM
viewer.destroy();
```

支持 `Blob`、`ArrayBuffer`、`Uint8Array`，提供 `open/clear/resize/destroy/on`、缩放和文字水印接口。Worker 和 WASM 默认从包内资源加载，也可以配置 `assets.baseUrl` 和 `workerFactory`。请在支持 WebAssembly、Web Worker 和 Shadow DOM 的浏览器中使用。

完整接入、静态资源、取消和错误处理说明见仓库的 [docs/sdk.md](https://github.com/wuqinglangzitao/office-view-suite/blob/main/docs/sdk.md)。支持通过 `options.watermark` 初始化水印，使用 `await viewer.setWatermark({ content: ['内部资料'] })` 更新，使用 `await viewer.setWatermark(null)` 关闭。水印不修改原文件。

## 许可证

项目原创代码采用 Apache-2.0，完整条款随包提供于 `dist/LICENSE`，允许商业使用和闭源应用集成。分发时应保留许可证及适用的版权、署名和修改声明。各格式适配器中的第三方依赖遵循各自许可证，声明随包提供于 `dist/THIRD_PARTY_NOTICES.md` 和 `dist/licenses/`；业务样例素材另行授权。
