# Vue 2 + Vite 接入

已在 Chrome 中实测 Vue **2.7.16**、Vite **6.4.4**、`@vitejs/plugin-vue2` **2.3.4**。开发服务器与生产构建均通过，SDK 从六个 `.tgz` 包独立安装，无源码别名；应用部署在 `/vue2/` 子路径。

## 运行示例

在仓库根目录执行：

```sh
pnpm install
pnpm wasm       # 首次或 Rust 构建输入变化时需要
pnpm dev:vue2
```

访问终端输出地址的 `/vue2/` 路径。默认使用包内 Worker/WASM；追加 `?assets=static` 可验证固定静态目录。示例位于 [fixtures/vue2-vite](../fixtures/vue2-vite)，SDK 未增加 Vue 运行时依赖。

## 安装与 Vite 配置

SDK 尚未发布 npm。先按 [构建与安装](sdk.md#构建与安装) 生成六个安装包，复制验证消费者的 `tarballs/`，将其 `dependencies` 和 `pnpm.overrides` 中的六个本地包映射合并到业务项目，再执行 `pnpm install`。已有 Vue 应用应保留自己的 Vue 依赖。

在已有 Vite 配置中合并以下内容：

```js
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue2';

export default defineConfig({
  plugins: [vue()],
  optimizeDeps: {
    exclude: [
      '@office-view-suite/viewer',
      '@office-view-suite/viewer-core',
      '@office-view-suite/viewer-word',
      '@office-view-suite/viewer-excel',
      '@office-view-suite/viewer-ppt',
      '@office-view-suite/viewer-pdf',
    ],
  },
  build: { target: 'es2022' },
});
```

开发模式下必须避免 SDK 被依赖预打包：实测未排除时，Worker 请求错误地指向 `node_modules/.vite/deps/assets/...` 并加载失败。排除后，Vite 可从包的真实目录提供资源；生产构建会输出独立 Worker/WASM。更新已有配置后应重启开发服务器，可使用 `vite --force` 刷新旧缓存。

官方 `@vitejs/plugin-vue2` 仅支持 Vue 2.7；Vue 2.6、其他插件及 Vite 版本不在本次实测范围。

## 使用组件

复制 [OfficePreview.vue](../fixtures/vue2-vite/src/OfficePreview.vue) 到业务项目。它使用 Vue 2 Options API，在 `mounted` 创建 SDK，在 `beforeDestroy` 销毁；实例不进入 `data`，通过替换 `source` 对象切换文件。

```vue
<template>
  <div>
    <input type="file" @change="selectFile" />
    <div style="height: 75vh">
      <OfficePreview :source="source" @error="onError" />
    </div>
  </div>
</template>

<script>
import OfficePreview from './OfficePreview.vue';

export default {
  components: { OfficePreview },
  data() { return { source: null }; },
  methods: {
    selectFile(event) {
      const file = event.target.files[0];
      this.source = file ? { bytes: file, name: file.name } : null;
    },
    onError(error) { console.error(error.code, error.message); },
  },
};
</script>
```

`source.bytes` 支持 `File`、`Blob`、`ArrayBuffer`、`Uint8Array`；远程 URL 需先 `fetch` 为这些类型。设 `source = null` 清空。`ready` 返回打开结果，`statechange` 返回 SDK 状态，`error` 同时覆盖打开失败和后台渲染失败，并对同一个错误去重。容器需要可测量宽度和外层高度；弹窗重新显示后可在 `$nextTick` 中调用组件的 `resize()`。

固定资源目录时，将四个渲染包的 `dist/assets/` 内容合并到 `public/office-assets/`，保留内部路径；组件传入 `:assets-base-url="assetsBaseUrl"`，其值为 `${import.meta.env.BASE_URL}office-assets/`。许可材料需一并部署，见 [许可分发](sdk.md#第三方许可随包部署)。

## 重复验证

```sh
pnpm build:sdk
pnpm test:packages
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs pnpm test:vue2
```

测试复用已安装的 Playwright 和 Chrome。首次安装隔离消费者时可能联网获取 Vue/Vite 工具依赖。每次建立新的临时项目，从 tarball 安装 SDK，检查开发/生产 × 默认/固定资源目录四种组合，共 44 项检查：四格式实际渲染、File 上传、缩放、隐藏后显示、加载中替换、清空、错误去重与恢复、加载中卸载及重新挂载、双实例和 Worker 释放。

结果与四种格式截图位于 `output/playwright/vue2/`，临时消费者路径记录于 `output/vue2-consumer.json`。这是框架接入验证，不表示与 Office 原软件像素级一致；未验证的旧 Vue 版本及其他浏览器仍需另测。
