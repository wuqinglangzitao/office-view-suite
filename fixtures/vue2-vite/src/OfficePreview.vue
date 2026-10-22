<template>
  <div class="office-preview">
    <p v-if="watermarkErrorMessage" class="office-preview__watermark-error" role="alert">{{ watermarkErrorMessage }}</p>
    <div ref="container" class="office-preview__host"></div>
  </div>
</template>

<script>
import { createOfficeViewer } from '@office-view-suite/viewer';

export default {
  name: 'OfficePreview',

  props: {
    // Replace the source object to open another file; bytes can also be a File.
    source: { type: Object, default: null },
    assetsBaseUrl: { type: String, default: '' },
    watermark: { type: Object, default: null },
  },

  data() {
    return { watermarkErrorMessage: '' };
  },

  watch: {
    source() { this.queueInputs(true); },
    // Replace the whole object; mutations inside the object are not observed.
    watermark() { this.queueInputs(false, true); },
  },

  created() {
    this._inputVersion = 0;
    this._needsOpen = false;
    this._clearRequired = false;
    this._watermarkDirty = false;
    this._flushQueued = false;
    this._disposed = false;
    this._watermarkPromise = null;
    this._watermarkPending = false;
    this._watermarkNeedsRetry = false;
    this._watermarkUpdateId = 0;
    this._requestedWatermark = this.watermark;
    this._reportedErrors = new WeakSet();
    this._reportedWatermarkErrors = new WeakSet();
  },

  mounted() {
    // Keep the SDK and error identities outside Vue 2's reactive data tree.
    if (!this.initializeViewer()) return;
    this._needsOpen = true;
    this.queueInputs();
  },

  activated() {
    this.$nextTick(() => this.resize());
  },

  beforeDestroy() {
    this._disposed = true;
    this._inputVersion += 1;
    this._unsubscribe?.();
    this._viewer?.destroy();
    this._viewer = null;
  },

  methods: {
    initializeViewer() {
      try {
        this._viewer = createOfficeViewer(this.$refs.container, {
          ...(this.assetsBaseUrl ? { assets: { baseUrl: this.assetsBaseUrl } } : {}),
          watermark: this.watermark,
        });
      } catch (error) {
        this._needsOpen = true;
        if (error.code === 'INVALID_WATERMARK') this.reportWatermarkError(error);
        this.reportError(error);
        return false;
      }
      this._unsubscribe = this._viewer.on(event => {
        if (event.type === 'error') this.reportError(event.error);
        else if (event.type === 'watermarkerror') this.reportWatermarkError(event.error);
        else if (event.type === 'statechange') this.$emit('statechange', event.state);
        else if (event.type === 'warning') this.$emit('warning', event);
      });
      return true;
    },

    queueInputs(sourceChanged = false, watermarkChanged = false) {
      if (this._disposed) return;
      this._inputVersion += 1;
      this._needsOpen = this._needsOpen || sourceChanged;
      this._clearRequired = this._clearRequired || sourceChanged;
      this._watermarkDirty = this._watermarkDirty || watermarkChanged;
      if (this._flushQueued) return;
      this._flushQueued = true;
      this.$nextTick(() => {
        this._flushQueued = false;
        void this.flushInputs();
      });
    },

    async flushInputs() {
      if (this._disposed) return;
      if (!this._viewer) {
        if (!this.initializeViewer()) return;
        // The constructor owns the first preparation; do not draw twice.
        this._watermarkDirty = false;
        this._watermarkPromise = null;
      }
      const viewer = this._viewer;
      const source = this.source;
      const version = this._inputVersion;
      if (!viewer || this._disposed) return;
      const shouldOpen = this._needsOpen;
      let cancelledWatermark = false;
      // Clear before changing identities, so a failed new watermark cannot
      // expose another attachment underneath the previous user's watermark.
      if (this._clearRequired) {
        cancelledWatermark = this._watermarkPending;
        this._clearRequired = false;
        viewer.clear();
      }
      try {
        if (this._watermarkDirty) {
          this._watermarkDirty = false;
          this._watermarkPromise = this.applyWatermark(this.watermark);
        } else if (shouldOpen && (this._watermarkNeedsRetry || cancelledWatermark)) {
          // A runtime failure retains the displayed tile. A later attachment
          // retries the desired identity rather than inheriting that old tile.
          this._watermarkPromise = this.applyWatermark(this._requestedWatermark);
        }
        if (this._watermarkPending && this._watermarkPromise) await this._watermarkPromise;
        if (this._disposed || this._viewer !== viewer || version !== this._inputVersion) return;
        if (!shouldOpen) return;
        this._needsOpen = false;
        if (!source) return;
        const result = await viewer.open(source);
        if (this._viewer === viewer && this.source === source) this.$emit('ready', result);
      } catch (error) {
        if (this._viewer !== viewer || version !== this._inputVersion) return;
        if (['INVALID_WATERMARK', 'WATERMARK_FAILED'].includes(error.code)) {
          this.reportWatermarkError(error);
          if (shouldOpen) this.reportError(error);
        } else this.reportError(error);
      }
    },

    async applyWatermark(value) {
      const updateId = ++this._watermarkUpdateId;
      this._requestedWatermark = value;
      this._watermarkPending = true;
      try {
        await this._viewer.setWatermark(value);
        if (updateId === this._watermarkUpdateId) {
          this._watermarkNeedsRetry = false;
          this.watermarkErrorMessage = '';
        }
      } catch (error) {
        if (updateId === this._watermarkUpdateId && !this._disposed) {
          this._watermarkNeedsRetry = true;
          this.reportWatermarkError(error);
        }
        throw error;
      } finally {
        if (updateId === this._watermarkUpdateId) this._watermarkPending = false;
      }
    },

    setWatermark(value) {
      if (!this._viewer || this._disposed) {
        return Promise.reject(Object.assign(new Error('预览组件未挂载或已销毁'), { code: 'DESTROYED' }));
      }
      if (this._clearRequired) {
        this._clearRequired = false;
        this._viewer.clear();
      }
      this._watermarkPromise = this.applyWatermark(value);
      // A source change already waiting for a watermark must follow this update.
      if (this._needsOpen) this.queueInputs();
      return this._watermarkPromise;
    },

    getWatermark() { return this._viewer?.getWatermark() ?? null; },

    reportWatermarkError(error) {
      if (['ABORTED', 'DESTROYED'].includes(error.code) || this._reportedWatermarkErrors.has(error)) return;
      this._reportedWatermarkErrors.add(error);
      this.watermarkErrorMessage = error.message || '水印更新失败';
      this.$emit('watermark-error', error);
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
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}
.office-preview__host { flex: 1; min-width: 0; min-height: 0; overflow: auto; }
.office-preview__watermark-error { flex: none; margin: 0 0 8px; color: #b42318; }
</style>
