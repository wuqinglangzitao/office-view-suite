import Vue from 'vue';
import AttachmentPreviewDialog from './AttachmentPreviewDialog.vue';

let activeInstance = null;

function previewError(code, message) {
  return Object.assign(new Error(message), { name: 'OfficeViewerError', code });
}

function rejectWatermarkRequest(instance, error) {
  const request = instance._watermarkRequest;
  instance._watermarkRequest = null;
  request?.reject(error);
}

function dispose(instance) {
  if (instance._disposedByService) return;
  instance._disposedByService = true;
  rejectWatermarkRequest(instance, previewError('DESTROYED', '附件预览已销毁'));
  if (activeInstance === instance) activeInstance = null;
  instance._detachOwner?.();
  instance.$destroy();
  instance.$el?.remove();
}

export function closeAttachmentPreview() {
  if (activeInstance) dispose(activeInstance);
}

// 全局单例入口；组件式接入可独立挂载多个弹窗。
export function openAttachmentPreview({ parent, i18n, ...props } = {}) {
  closeAttachmentPreview();
  const instance = new Vue({
    parent,
    ...(i18n ? { i18n } : {}),
    data() { return { dialogVisible: true, desiredWatermark: props.watermark ?? null }; },
    beforeDestroy() {
      rejectWatermarkRequest(this, previewError('DESTROYED', '附件预览已销毁'));
    },
    methods: {
      setWatermark(value) {
        if (this._disposedByService || this._isDestroyed || !this.dialogVisible) {
          return Promise.reject(previewError(this._disposedByService || this._isDestroyed ? 'DESTROYED' : 'ABORTED', '附件预览已关闭'));
        }
        rejectWatermarkRequest(this, previewError('ABORTED', '水印更新已被替代'));
        const promise = new Promise((resolve, reject) => {
          this._watermarkRequest = { resolve, reject };
        });
        void promise.catch(() => {});
        const request = this._watermarkRequest;
        const apply = () => {
          if (this._watermarkRequest !== request || this._disposedByService || !this.dialogVisible) return;
          const dialog = this.$refs.preview;
          if (!dialog) {
            rejectWatermarkRequest(this, previewError('DESTROYED', '附件预览组件不可用'));
            return;
          }
          // Set the child's intent before changing the prop. Its watcher then
          // observes the same object and does not submit a duplicate update.
          const applied = dialog.setWatermark(value);
          this.desiredWatermark = value;
          void applied.then(() => {
            if (this._watermarkRequest !== request) return;
            this._watermarkRequest = null;
            request.resolve();
          }, error => {
            if (this._watermarkRequest !== request) return;
            this._watermarkRequest = null;
            const previous = dialog.getWatermark();
            dialog.syncWatermarkIntent(previous);
            this.desiredWatermark = previous;
            request.reject(error);
          });
        };
        if (this.$refs.preview) apply();
        else {
          this.desiredWatermark = value;
          this.$nextTick(apply);
        }
        return promise;
      },
      getWatermark() { return this.$refs.preview?.getWatermark() ?? null; },
    },
    render(h) {
      return h(AttachmentPreviewDialog, {
        ref: 'preview',
        props: { ...props, visible: this.dialogVisible, watermark: this.desiredWatermark },
        on: {
          'update:visible': visible => {
            if (activeInstance === instance) {
              this.dialogVisible = visible;
              if (!visible) rejectWatermarkRequest(this, previewError('ABORTED', '附件预览已关闭'));
            }
          },
          closed: () => {
            if (activeInstance === instance) dispose(instance);
          },
          ready: result => this.$emit('ready', result),
          error: error => this.$emit('error', error),
          'download-error': error => this.$emit('download-error', error),
          'watermark-error': error => this.$emit('watermark-error', error),
        },
      });
    },
  });
  activeInstance = instance;
  if (parent) {
    const onOwnerDestroy = () => dispose(instance);
    parent.$once('hook:beforeDestroy', onOwnerDestroy);
    instance._detachOwner = () => parent.$off('hook:beforeDestroy', onOwnerDestroy);
  }
  try {
    instance.$mount();
    document.body.appendChild(instance.$el);
    return instance;
  } catch (error) {
    dispose(instance);
    throw error;
  }
}
