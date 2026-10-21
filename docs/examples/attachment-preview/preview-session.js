import { createOfficeViewer } from '@office-view-suite/viewer';

export function isCancelled(error) {
  return error?.name === 'AbortError' || ['ABORTED', 'DESTROYED'].includes(error?.code);
}

// 自定义请求层需实现相同签名，返回 Blob / ArrayBuffer / Uint8Array。
export async function fetchAttachment({ fileUrl, signal }) {
  const response = await fetch(fileUrl, { signal, credentials: 'same-origin' });
  if (!response.ok) throw new Error(`附件请求失败：HTTP ${response.status}`);
  return response.blob();
}

export function createPreviewSession(container, {
  fileUrl,
  fileName,
  assetsBaseUrl = '',
  fetchFile = fetchAttachment,
  onError = () => {},
}) {
  const controller = new AbortController();
  const viewer = createOfficeViewer(container, assetsBaseUrl
    ? { assets: { baseUrl: assetsBaseUrl } }
    : undefined);
  let removed = false;
  const unsubscribe = viewer.on(event => {
    if (!removed && event.type === 'error' && !isCancelled(event.error)) {
      onError(event.error);
    }
  });
  const ensureActive = () => {
    if (removed) throw new DOMException('附件预览已取消', 'AbortError');
  };

  const ready = (async () => {
    const bytes = await fetchFile({ fileUrl, fileName, signal: controller.signal });
    ensureActive();
    const result = await viewer.open({ bytes, name: fileName, signal: controller.signal });
    ensureActive();
    viewer.setZoom(['xlsx', 'xlsm'].includes(result.format) ? 1 : 'fit-width');
    return result;
  })();

  return {
    ready,
    resize() { if (!removed) viewer.resize(); },
    setZoom(value) { if (!removed) viewer.setZoom(value); },
    getZoom() { return viewer.getZoom(); },
    remove() {
      if (removed) return;
      removed = true;
      controller.abort();
      unsubscribe();
      viewer.destroy();
    },
  };
}

export function getFileName(fileName, fileUrl) {
  if (fileName) return fileName;
  try {
    const path = new URL(fileUrl, window.location.href).pathname;
    return decodeURIComponent(path.split('/').pop()) || 'attachment';
  } catch {
    return 'attachment';
  }
}

export function saveBlob(blob, fileName) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  try {
    document.body.appendChild(link);
    link.click();
  } finally {
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
}
