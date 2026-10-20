<template>
  <div ref="container" class="office-preview"></div>
</template>

<script>
import { createOfficeViewer } from '@office-view-suite/viewer';

export default {
  name: 'OfficePreview',

  props: {
    // Replace the source object to open another file; bytes can also be a File.
    source: { type: Object, default: null },
    assetsBaseUrl: { type: String, default: '' },
  },

  watch: {
    source: 'openSource',
  },

  mounted() {
    // Keep the SDK and error identities outside Vue 2's reactive data tree.
    this._reportedErrors = new WeakSet();
    this._viewer = createOfficeViewer(this.$refs.container, this.assetsBaseUrl
      ? { assets: { baseUrl: this.assetsBaseUrl } }
      : {});
    this._unsubscribe = this._viewer.on(event => {
      if (event.type === 'error') this.reportError(event.error);
      else if (event.type === 'statechange') this.$emit('statechange', event.state);
      else if (event.type === 'warning') this.$emit('warning', event);
    });
    this.openSource();
  },

  activated() {
    this.$nextTick(() => this.resize());
  },

  beforeDestroy() {
    this._unsubscribe?.();
    this._viewer?.destroy();
    this._viewer = null;
  },

  methods: {
    async openSource() {
      const viewer = this._viewer;
      const source = this.source;
      if (!viewer) return;
      if (!source) {
        viewer.clear();
        return;
      }
      try {
        const result = await viewer.open(source);
        if (this._viewer === viewer && this.source === source) this.$emit('ready', result);
      } catch (error) {
        if (this._viewer === viewer && this.source === source) this.reportError(error);
      }
    },

    reportError(error) {
      if (['ABORTED', 'DESTROYED'].includes(error.code) || this._reportedErrors.has(error)) return;
      this._reportedErrors.add(error);
      this.$emit('error', error);
    },

    resize() {
      this._viewer?.resize();
    },

    setZoom(value) {
      this._viewer?.setZoom(value);
    },
  },
};
</script>

<style scoped>
.office-preview {
  width: 100%;
  height: 100%;
  min-width: 0;
  overflow: auto;
}
</style>
