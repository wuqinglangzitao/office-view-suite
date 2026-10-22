<template>
  <main>
    <h1>Vue 2 文件预览</h1>
    <input id="file" type="file" accept=".doc,.docx,.docm,.xlsx,.xlsm,.pptx,.pdf" @change="selectFile" />
    <nav>
      <button v-for="kind in kinds" :key="kind" :data-sample="kind" @click="selectSample(kind)">{{ kind }}</button>
      <button id="clear" @click="clear">清空</button>
      <button id="visibility" @click="toggleVisibility">{{ visible ? '隐藏' : '显示' }}</button>
      <button id="mount" @click="previewMounted = !previewMounted">{{ previewMounted ? '卸载' : '挂载' }}</button>
      <button id="zoom" :disabled="!previewMounted" @click="$refs.preview.setZoom(1.5)">150%</button>
      <button id="fit" :disabled="!previewMounted" @click="$refs.preview.setZoom('fit-width')">适应宽度</button>
      <button id="second" @click="toggleSecond">第二实例</button>
    </nav>
    <form class="watermark-controls" @submit.prevent="applyWatermark">
      <fieldset>
        <legend>水印配置（主预览）</legend>
        <label class="watermark-content">
          内容（1–4 行）
          <textarea id="watermark-content" v-model="watermarkDraft.content" rows="2" required></textarea>
        </label>
        <div class="watermark-fields">
          <label>字号（px）<input id="watermark-font-size" v-model.number="watermarkDraft.fontSize" type="number" min="8" max="72" required /></label>
          <label>字体<input id="watermark-font-family" v-model="watermarkDraft.fontFamily" required /></label>
          <label>角度（°）<input id="watermark-rotate" v-model.number="watermarkDraft.rotate" type="number" min="-180" max="180" required /></label>
          <label>透明度<input id="watermark-opacity" v-model.number="watermarkDraft.opacity" type="number" min="0" max="1" step="0.01" required /></label>
          <label>颜色<input id="watermark-color" v-model="watermarkDraft.color" type="color" /></label>
          <label>水平间距（px）<input id="watermark-gap-x" v-model.number="watermarkDraft.gapX" type="number" min="0" max="512" required /></label>
          <label>垂直间距（px）<input id="watermark-gap-y" v-model.number="watermarkDraft.gapY" type="number" min="0" max="512" required /></label>
        </div>
        <div class="watermark-actions">
          <button id="apply-watermark" type="submit">应用 / 更新水印</button>
          <button id="disable-watermark" type="button" :disabled="!watermark" @click="disableWatermark">关闭水印</button>
        </div>
        <p>默认关闭。应用时替换完整配置对象，不重新打开文件；仅影响预览，下载仍为原文件。</p>
      </fieldset>
    </form>
    <output id="status" aria-live="polite">{{ status }}</output>
    <div class="viewers">
      <section v-show="visible" id="primary" class="preview-panel">
        <OfficePreview
          v-if="previewMounted"
          ref="preview"
          :source="source"
          :watermark="watermark"
          :assets-base-url="assetsBaseUrl"
          @ready="onReady"
          @statechange="onState"
          @error="onError"
          @watermark-error="onWatermarkError"
        />
      </section>
      <section v-if="secondMounted" id="secondary" class="preview-panel">
        <OfficePreview :source="secondSource" :assets-base-url="assetsBaseUrl" @ready="secondResult = $event" @error="onError" />
      </section>
    </div>
  </main>
</template>

<script>
import OfficePreview from './OfficePreview.vue';

const samples = { word: 'styles-word.docx', excel: 'styles-excel.xlsx', ppt: 'styles-ppt.pptx', pdf: 'handmade-mixed.pdf' };

async function sample(kind) {
  const name = samples[kind];
  const response = await fetch(`${import.meta.env.BASE_URL}samples/${name}`);
  if (!response.ok) throw new Error(`Sample request failed: ${response.status}`);
  return { bytes: await response.arrayBuffer(), name };
}

export default {
  components: { OfficePreview },

  data() {
    return {
      kinds: Object.keys(samples),
      source: null,
      watermark: null,
      watermarkDraft: {
        content: '内部资料 · 张三（00123）\n运输部 · 2026-10-22',
        fontSize: 14,
        fontFamily: 'sans-serif',
        rotate: -25,
        opacity: 0.12,
        color: '#000000',
        gapX: 120,
        gapY: 80,
      },
      watermarkErrors: [],
      secondSource: null,
      secondResult: null,
      previewMounted: true,
      secondMounted: false,
      visible: true,
      ready: [],
      errors: [],
      states: [],
      status: '选择文件或样例',
      assetsBaseUrl: new URLSearchParams(location.search).get('assets') === 'static'
        ? `${import.meta.env.BASE_URL}office-assets/` : '',
    };
  },

  methods: {
    sample,

    applyWatermark() {
      const draft = this.watermarkDraft;
      // Replace the prop object so the component's watcher applies this update.
      this.watermark = {
        content: draft.content,
        fontSize: draft.fontSize,
        fontFamily: draft.fontFamily,
        rotate: draft.rotate,
        opacity: draft.opacity,
        color: draft.color,
        gap: [draft.gapX, draft.gapY],
      };
    },

    disableWatermark() {
      this.watermark = null;
    },

    async selectSample(kind) {
      const selection = this._selection = (this._selection || 0) + 1;
      try {
        const source = await sample(kind);
        if (this._selection === selection) this.source = source;
      } catch (error) { this.onError(error); }
    },

    selectFile(event) {
      this._selection = (this._selection || 0) + 1;
      const file = event.target.files[0];
      this.source = file ? { bytes: file, name: file.name } : null;
    },

    clear() {
      this._selection = (this._selection || 0) + 1;
      this.source = null;
    },

    async toggleVisibility() {
      this.visible = !this.visible;
      await this.$nextTick();
      if (this.visible) this.$refs.preview?.resize();
    },

    async toggleSecond() {
      if (this.secondMounted) this.secondMounted = false;
      else {
        this.secondSource = await sample('pdf');
        this.secondMounted = true;
      }
    },

    onReady(result) {
      this.ready.push(result);
      this.status = `已打开 ${result.format}`;
    },

    onState(state) {
      this.states.push(state);
      this.status = state;
    },

    onError(error) {
      this.errors.push({ code: error.code, message: error.message });
      this.status = error.message;
    },

    onWatermarkError(error) {
      this.watermarkErrors.push({ code: error.code, message: error.message });
    },
  },
};
</script>

<style>
body { margin: 24px; background: #f5f5f5; font: 14px system-ui; }
nav { display: flex; flex-wrap: wrap; gap: 8px; margin: 16px 0; }
output { display: block; margin-bottom: 16px; }
.watermark-controls { margin-bottom: 16px; }
.watermark-controls fieldset { margin: 0; padding: 12px; border: 1px solid #ccc; }
.watermark-controls label { display: flex; flex-direction: column; gap: 4px; }
.watermark-content textarea { box-sizing: border-box; width: 100%; resize: vertical; font: inherit; }
.watermark-fields { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 12px; }
.watermark-fields input { box-sizing: border-box; width: 132px; height: 30px; }
.watermark-actions { display: flex; gap: 8px; margin-top: 12px; }
.watermark-controls p { margin: 8px 0 0; color: #555; }
.viewers { display: flex; gap: 20px; }
.preview-panel { flex: 1; min-width: 0; height: 680px; }
</style>
