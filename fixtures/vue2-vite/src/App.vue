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
.viewers { display: flex; gap: 20px; }
.preview-panel { flex: 1; min-width: 0; height: 680px; }
</style>
