# @office-view-suite/viewer

浏览器端 Word、Excel、PowerPoint 和 PDF 预览 SDK。原生 DOM、Shadow DOM、Worker、WASM，无 iframe。

```ts
import { createOfficeViewer } from '@office-view-suite/viewer';

const viewer = createOfficeViewer(document.querySelector<HTMLElement>('#preview')!);
await viewer.open({ bytes: file, name: file.name });
// 组件卸载时释放 Worker、观察器和预览 DOM
viewer.destroy();
```

支持 `Blob`、`ArrayBuffer`、`Uint8Array`，提供 `open/clear/resize/destroy/on`。Worker 和 WASM 默认从包内资源加载，也可以配置 `assets.baseUrl` 和 `workerFactory`。请在支持 WebAssembly、Web Worker 和 Shadow DOM 的浏览器中使用。

完整接入、静态资源、取消和错误处理说明见仓库的 [docs/sdk.md](https://github.com/wuqinglangzitao/office-view-suite/blob/main/docs/sdk.md)。当前为本地构建版本，尚未发布 npm。
