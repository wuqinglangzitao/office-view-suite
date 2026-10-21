import Vue from 'vue';
import AttachmentPreviewDialog from './AttachmentPreviewDialog.vue';

let activeInstance = null;

function dispose(instance) {
  if (instance._disposedByService) return;
  instance._disposedByService = true;
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
    data() { return { dialogVisible: true }; },
    render(h) {
      return h(AttachmentPreviewDialog, {
        props: { ...props, visible: this.dialogVisible },
        on: {
          'update:visible': visible => {
            if (activeInstance === instance) this.dialogVisible = visible;
          },
          closed: () => {
            if (activeInstance === instance) dispose(instance);
          },
          ready: result => this.$emit('ready', result),
          error: error => this.$emit('error', error),
          'download-error': error => this.$emit('download-error', error),
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
