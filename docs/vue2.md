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

SDK 支持 npm 公共仓库和公司私服；以下使用公网示例。若项目默认使用其他 registry，先按 [构建与安装](sdk.md#构建与安装) 配置项目 `.npmrc` 中的 `@office-view-suite` 作用域 registry，再安装公共入口：

```sh
pnpm add @office-view-suite/viewer@0.2.0
```

其余五个 SDK 包会自动安装，已有 Vue 应用应保留自己的 Vue 依赖。离线交付时，也可按上述文档生成六个安装包，复制验证消费者的 `tarballs/`，将其 `dependencies` 和 `pnpm.overrides` 中的六个本地包映射合并到业务项目，再执行 `pnpm install`。

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
  build: {
    target: 'es2022',
    assetsInlineLimit(filePath) {
      if (filePath.includes('/node_modules/@office-view-suite/')) return false;
      return undefined;
    },
  },
});
```

开发模式下必须避免 SDK 被依赖预打包：实测未排除时，Worker 请求错误地指向 `node_modules/.vite/deps/assets/...` 并加载失败。排除后，Vite 可从包的真实目录提供资源；生产构建会输出独立 Worker/WASM。更新已有配置后应重启开发服务器，可使用 `vite --force` 刷新旧缓存。

`assetsInlineLimit` 让 SDK 资源保持为独立文件，避免 PDF classic Worker 被内联为失去同源上下文的 data URL；其他应用资源沿用默认内联阈值。

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

## 文字水印（源码 0.2.0）

运行示例中的“水印配置（主预览）”可编辑多行内容、字号、字体、角度、透明度、颜色及水平/垂直间距，点击“应用 / 更新水印”生效，点击“关闭水印”移除。默认关闭；可以先配置再选择文件，也可以在阅读时更新。

下面是可复制的 Options API 示例，初始水印在打开文件前配置；修改姓名或部门后点击“应用 / 更新水印”，即可替换身份信息：

```vue
<template>
  <div>
    <input type="file" accept=".doc,.docx,.docm,.xlsx,.xlsm,.pptx,.pdf" @change="selectFile" />
    <label>姓名 <input v-model="userName" /></label>
    <label>部门 <input v-model="department" /></label>
    <button @click="updateWatermark">应用 / 更新水印</button>
    <button @click="disableWatermark">关闭水印</button>
    <div style="height: 75vh; min-width: 0">
      <OfficePreview
        ref="preview"
        :source="source"
        :watermark="watermark"
        @error="onError"
        @watermark-error="onWatermarkError"
      />
    </div>
  </div>
</template>

<script>
import OfficePreview from './OfficePreview.vue';

function createWatermark(userName, department) {
  return {
    content: [
      `内部资料 · ${userName}`,
      `${department} · ${new Date().toLocaleDateString('zh-CN')}`,
    ],
    rotate: -25,
    opacity: 0.12,
    fontSize: 14,
    fontFamily: 'sans-serif',
    color: '#000000',
    gap: [120, 80],
  };
}

export default {
  components: { OfficePreview },
  data() {
    return {
      source: null,
      userName: '张三（00123）',
      department: '运输部',
      watermark: createWatermark('张三（00123）', '运输部'),
    };
  },
  methods: {
    selectFile(event) {
      const file = event.target.files[0];
      this.source = file ? { bytes: file, name: file.name } : null;
    },
    updateWatermark() {
      // 替换整个对象；仅修改 this.watermark.content 不会触发更新。
      this.watermark = createWatermark(this.userName, this.department);
    },
    disableWatermark() {
      this.watermark = null;
    },
    onError(error) { console.error(error.code, error.message); },
    onWatermarkError(error) { console.warn('水印配置或绘制失败', error.code, error.message); },
  },
};
</script>
```

`content` 支持字符串或字符串数组，换行后最多 4 行；`opacity` 范围为 0–1，设为 0 只是透明，关闭请用 `null`。`gap` 顺序为水平、垂直间距（CSS px）。参数范围与 CSP 要求见[文字水印](usage.md#文字水印020)。示例中的身份信息请替换为业务登录用户数据。

组件不使用深度监听，更新时必须替换整个 `watermark` 对象；SDK 按完整配置替换，省略的样式恢复默认值。只更新水印不重新打开文件，也不重置缩放或滚动位置。source 和水印同轮替换时，组件先清空、等待新水印成功，再展示新文件。

需要等待提交完成时，可在组件挂载后使用命令式接口；该方式与 prop 绑定任选一种作为配置来源，避免两套状态相互覆盖：

```js
try {
  await this.$refs.preview.setWatermark({ content: ['内部资料 · 李四', '运输部'], opacity: 0.18 });
  console.log(this.$refs.preview.getWatermark()); // 已成功应用的完整配置
  await this.$refs.preview.setWatermark(null);   // 关闭
} catch (error) {
  console.warn(error.code, error.message);
}
```

prop 更新需要等待 Vue watcher 和异步绘制完成；`$nextTick()` 本身不表示水印已应用。`getWatermark()` 返回成功应用的快照；命令式 setter 不会反向修改父组件的 prop。

`watermark-error` 独立于预览错误；运行时水印更新失败会保留旧水印，不禁用已有文档的缩放或下载。附件服务 `openAttachmentPreview({ ..., watermark })` 返回的包装实例同样提供异步 setter/getter，setter 等待真实提交；关闭时取消待处理更新。水印仅影响预览，下载仍为原文件。私服 `0.1.2` 不含这些接口，请使用公网 `0.2.0` 或同版本源码打包产物。

## 重复验证

```sh
pnpm build:sdk
pnpm test:packages
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs pnpm test:vue2
```

测试复用已安装的 Playwright 和 Chrome。首次安装隔离消费者时可能联网获取 Vue/Vite 工具依赖。每次建立新的临时项目，从 tarball 安装 SDK，检查开发/生产 × 默认/固定资源目录四种组合，共 56 项检查：四格式实际渲染、File 上传、缩放、隐藏后显示、加载中替换、清空、错误去重与恢复、水印动态更新与身份切换、加载中卸载及重新挂载、双实例和 Worker 释放。

`pnpm test:attachment` 另建临时消费者安装 Element UI 2.15.3，执行开发/生产两套共 8 项附件服务检查。组件库仅用于验证消费者，不加入 SDK 运行时依赖。水印几何与 Canvas/CSP 验证可执行 `pnpm test:watermark:layout` 和 `pnpm test:watermark`；这些浏览器命令均使用同一 `PLAYWRIGHT_MODULE` 设置。

结果与四种格式截图位于 `output/playwright/vue2/`，临时消费者路径记录于 `output/vue2-consumer.json`。这是框架接入验证，不表示与 Office 原软件像素级一致；未验证的旧 Vue 版本及其他浏览器仍需另测。
