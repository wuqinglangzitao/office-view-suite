<template>
  <el-dialog
    custom-class="ovs-attachment-dialog"
    :visible="visible"
    width="94%"
    top="3vh"
    append-to-body
    @update:visible="$emit('update:visible', $event)"
    @opened="handleOpened"
    @closed="handleClosed"
  >
    <div slot="title" class="ovs-attachment-dialog__title">
      <span class="ovs-attachment-dialog__name">{{ fileName || title }}</span>
      <el-button size="small" :loading="downloading" :disabled="!fileUrl" @click="download">
        下载原文件
      </el-button>
    </div>

    <div class="ovs-attachment-dialog__content">
      <div class="ovs-attachment-dialog__toolbar" role="toolbar" aria-label="文档缩放">
        <el-button size="small" :disabled="!canZoom || zoomScale <= 0.1" @click="changeZoom(-0.1)">缩小</el-button>
        <output class="ovs-attachment-dialog__zoom" aria-live="polite">{{ zoomLabel }}</output>
        <el-button size="small" :disabled="!canZoom || zoomScale >= 4" @click="changeZoom(0.1)">放大</el-button>
        <el-button size="small" :disabled="!canZoom" @click="setZoom(1)">还原</el-button>
      </div>
      <p v-if="loading" class="ovs-attachment-dialog__message" role="status">正在加载文档…</p>
      <p v-if="errorMessage" class="ovs-attachment-dialog__error" role="alert">{{ errorMessage }}</p>
      <p v-if="watermarkError" class="ovs-attachment-dialog__error" role="alert">{{ watermarkError }}</p>
      <p v-if="downloadError" class="ovs-attachment-dialog__error" role="alert">{{ downloadError }}</p>
      <div ref="officeHost" class="ovs-attachment-dialog__host" :aria-busy="loading"></div>
    </div>
  </el-dialog>
</template>

<script>
import {
  createPreviewSession,
  fetchAttachment,
  getFileName,
  isCancelled,
  saveBlob,
} from './preview-session.js';

export default {
  name: 'AttachmentPreviewDialog',
  props: {
    visible: { type: Boolean, default: false },
    fileUrl: { type: String, default: '' },
    fileName: { type: String, default: '' },
    title: { type: String, default: '附件预览' },
    assetsBaseUrl: { type: String, default: '' },
    fetchFile: { type: Function, default: fetchAttachment },
    downloadHandler: { type: Function, default: null },
    watermark: { type: Object, default: null },
  },
  data() {
    return {
      loading: false,
      errorMessage: '',
      downloadError: '',
      watermarkError: '',
      downloading: false,
      zoomScale: null,
    };
  },
  computed: {
    canZoom() { return !this.loading && !this.errorMessage && this.zoomScale !== null; },
    zoomLabel() { return this.zoomScale === null ? '—' : `${Math.round(this.zoomScale * 100)}%`; },
  },
  watch: {
    visible(value) {
      if (value) this.queuePreview();
      else this.cleanup();
    },
    fileUrl() { this.queuePreview(); },
    fileName() { this.queuePreview(); },
    assetsBaseUrl() { this.queuePreview(); },
    fetchFile() { this.queuePreview(); },
    watermark(value) {
      // A service call sets the intent before updating its prop; avoid applying twice.
      if (value !== this._watermarkInput) void this.setWatermark(value).catch(() => {});
    },
  },
  created() {
    // SDK、AbortController 不放入 data，避免 Vue 2 深度观察它们。
    this._previewSession = null;
    this._downloadController = null;
    this._requestId = 0;
    this._disposed = false;
    this._watermarkVersion = 0;
    this._watermarkInput = this.watermark;
    this._desiredWatermark = this.watermark;
    this._watermarkRequest = null;
    this._lastWatermarkPromise = null;
    this._reportedWatermarkErrors = new WeakSet();
  },
  mounted() {
    this._resizeObserver = new ResizeObserver(() => {
      cancelAnimationFrame(this._resizeFrame);
      this._resizeFrame = requestAnimationFrame(() => this.resize());
    });
    this.queuePreview();
  },
  beforeDestroy() {
    this._disposed = true;
    this.cleanup();
    this._resizeObserver?.disconnect();
    cancelAnimationFrame(this._resizeFrame);
  },
  methods: {
    async queuePreview() {
      // Source watchers coalesce through nextTick. Keep a lazy setter waiting
      // for the new session, rather than applying it to an outgoing document.
      this.cleanup(true);
      if (this._disposed || !this.visible || !this.fileUrl) return;
      const requestId = this._requestId;
      this.loading = true;
      await this.$nextTick();
      if (requestId !== this._requestId || this._disposed || !this.visible) return;
      const host = this.$refs.officeHost;
      // el-dialog 默认懒挂载；opened 事件会在宿主就绪时重试。
      if (!host || !host.clientWidth) return;
      this._resizeObserver.disconnect();
      this._resizeObserver.observe(host);
      try {
        const watermarkVersion = this._watermarkVersion;
        const session = createPreviewSession(host, {
          fileUrl: this.fileUrl,
          fileName: getFileName(this.fileName, this.fileUrl),
          assetsBaseUrl: this.assetsBaseUrl,
          fetchFile: this.fetchFile,
          watermark: this._desiredWatermark,
          onError: error => {
            if (requestId === this._requestId) this.reportError(error);
          },
          onWatermarkError: error => {
            if (requestId === this._requestId) this.reportWatermarkError(error);
          },
        });
        this._previewSession = session;
        void session.watermarkReady.then(() => {
          if (requestId === this._requestId && watermarkVersion === this._watermarkVersion) this.completeWatermark();
        }, error => {
          if (requestId === this._requestId && watermarkVersion === this._watermarkVersion) {
            this.failWatermark(error);
            this.reportWatermarkError(error);
          }
        });
        const result = await session.ready;
        if (requestId !== this._requestId) return;
        this.syncZoom();
        this.$emit('ready', result);
      } catch (error) {
        if (requestId === this._requestId) {
          if (!this._previewSession) this.failWatermark(error);
          if (['INVALID_WATERMARK', 'WATERMARK_FAILED'].includes(error.code)) this.reportWatermarkError(error);
          this.reportError(error);
        }
      } finally {
        if (requestId === this._requestId) this.loading = false;
      }
    },
    handleOpened() {
      if (this._previewSession) this.resize();
      else this.queuePreview();
    },
    handleClosed() {
      // 旧关闭动画结束时，弹窗可能已经打开了另一份附件。
      if (!this.visible) {
        this.cleanup();
        this.$emit('closed');
      }
    },
    cleanup(preserveWatermarkRequest = false) {
      this._requestId += 1;
      this._previewSession?.remove();
      this._previewSession = null;
      this._downloadController?.abort();
      this._downloadController = null;
      this._resizeObserver?.disconnect();
      this.loading = false;
      this.downloading = false;
      this.zoomScale = null;
      this.errorMessage = '';
      this.downloadError = '';
      this.watermarkError = '';
      if (!preserveWatermarkRequest) {
        this._watermarkVersion += 1;
        this.failWatermark(Object.assign(new Error(this._disposed ? '附件预览已销毁' : '附件预览已关闭'), {
          code: this._disposed ? 'DESTROYED' : 'ABORTED',
        }));
        this._lastWatermarkPromise = null;
      }
    },
    setWatermark(value) {
      if (this._disposed || !this.visible) {
        return Promise.reject(Object.assign(new Error('附件预览未打开'), { code: this._disposed ? 'DESTROYED' : 'ABORTED' }));
      }
      if (value === this._watermarkInput && this._lastWatermarkPromise) return this._lastWatermarkPromise;
      this._watermarkVersion += 1;
      const version = this._watermarkVersion;
      this.failWatermark(Object.assign(new Error('水印更新已被替代'), { code: 'ABORTED' }));
      this._watermarkInput = value;
      this._desiredWatermark = value;
      this.watermarkError = '';
      const promise = new Promise((resolve, reject) => {
        this._watermarkRequest = { version, resolve, reject };
      });
      this._lastWatermarkPromise = promise;
      // Attach a rejection handler even when a prop watcher owns the request.
      void promise.catch(() => {});
      const session = this._previewSession;
      if (session) {
        void session.setWatermark(value).then(() => {
          if (session === this._previewSession && version === this._watermarkVersion) this.completeWatermark();
        }, error => {
          // A source replacement will apply the pending intent on its new session.
          if (session !== this._previewSession || version !== this._watermarkVersion) return;
          this.failWatermark(error);
          this.reportWatermarkError(error);
        });
      }
      // No session yet: queuePreview/handleOpened uses the latest intent in its
      // constructor. The promise completes when its watermark barrier commits.
      return promise;
    },
    getWatermark() { return this._previewSession?.getWatermark() ?? null; },
    // Service-owned state can roll back after failure; ordinary props are never mutated.
    syncWatermarkIntent(value) {
      this._watermarkInput = value;
      this._desiredWatermark = value;
      this._lastWatermarkPromise = null;
      // Invalid updates do not cancel an earlier valid SDK task. Restore the
      // service-owned intent explicitly so an older pending user cannot commit
      // after the service has rolled back its state to the applied snapshot.
      const session = this._previewSession;
      if (session) {
        void session.setWatermark(value).catch(error => {
          if (session === this._previewSession) this.reportWatermarkError(error);
        });
      }
    },
    completeWatermark() {
      const request = this._watermarkRequest;
      this._watermarkRequest = null;
      this.watermarkError = '';
      request?.resolve();
    },
    failWatermark(error) {
      const request = this._watermarkRequest;
      this._watermarkRequest = null;
      request?.reject(error);
    },
    reportWatermarkError(error) {
      if (isCancelled(error) || this._reportedWatermarkErrors.has(error)) return;
      this._reportedWatermarkErrors.add(error);
      this.watermarkError = error?.message || '水印更新失败';
      this.$emit('watermark-error', error);
    },
    reportError(error) {
      if (isCancelled(error)) return;
      const message = error?.message || '附件预览失败';
      // open() 拒绝和 SDK error 事件可能报告同一个错误。
      if (this.errorMessage === message) return;
      this.errorMessage = message;
      this.$emit('error', error);
    },
    syncZoom() {
      if (this._previewSession) this.zoomScale = this._previewSession.getZoom().scale;
    },
    resize() {
      if (this._disposed || !this.visible || !this._previewSession) return;
      this._previewSession.resize();
      if (!this.loading) this.syncZoom();
    },
    setZoom(value) {
      if (!this.canZoom || !this._previewSession) return;
      try {
        this._previewSession.setZoom(value);
        this.syncZoom();
      } catch (error) {
        this.reportError(error);
      }
    },
    changeZoom(step) {
      this.setZoom(Math.min(4, Math.max(0.1, Number((this.zoomScale + step).toFixed(2)))));
    },
    async download() {
      if (this.downloading || !this.fileUrl) return;
      const controller = new AbortController();
      this._downloadController = controller;
      const request = {
        fileUrl: this.fileUrl,
        fileName: getFileName(this.fileName, this.fileUrl),
        signal: controller.signal,
      };
      this.downloading = true;
      this.downloadError = '';
      try {
        if (this.downloadHandler) {
          // 自定义回调自行完成下载；需传递 signal 才能取消其网络请求。
          await this.downloadHandler(request);
        } else {
          const bytes = await this.fetchFile(request);
          if (!controller.signal.aborted && this._downloadController === controller) {
            saveBlob(bytes instanceof Blob ? bytes : new Blob([bytes]), request.fileName);
          }
        }
      } catch (error) {
        if (!controller.signal.aborted && this._downloadController === controller && !isCancelled(error)) {
          this.downloadError = error?.message || '附件下载失败';
          this.$emit('download-error', error);
        }
      } finally {
        if (this._downloadController === controller) {
          this._downloadController = null;
          this.downloading = false;
        }
      }
    },
  },
};
</script>

<!-- append-to-body 后仍需作用于 el-dialog 内部；所有选择器限定唯一前缀。 -->
<style>
.ovs-attachment-dialog.el-dialog {
  display: flex;
  flex-direction: column;
  height: 94vh;
  max-width: 1200px;
  margin-bottom: 0;
  border-radius: 0;
}
.ovs-attachment-dialog > .el-dialog__header { flex: none; padding-right: 48px; }
.ovs-attachment-dialog > .el-dialog__body {
  display: flex;
  flex: 1;
  min-height: 0;
  padding: 12px 20px 20px;
}
.ovs-attachment-dialog__title { display: flex; align-items: center; gap: 12px; }
.ovs-attachment-dialog__name { flex: 1; min-width: 0; overflow-wrap: anywhere; }
.ovs-attachment-dialog__content { display: flex; flex: 1; flex-direction: column; min-width: 0; min-height: 0; }
.ovs-attachment-dialog__toolbar { display: flex; flex: none; align-items: center; gap: 8px; margin-bottom: 10px; }
.ovs-attachment-dialog__toolbar .el-button + .el-button { margin-left: 0; }
.ovs-attachment-dialog__zoom { min-width: 52px; text-align: center; }
.ovs-attachment-dialog__message, .ovs-attachment-dialog__error { flex: none; margin: 0 0 10px; }
.ovs-attachment-dialog__error { color: #b42318; }
.ovs-attachment-dialog__host { flex: 1; width: 100%; min-width: 0; min-height: 0; overflow: auto; }
</style>
