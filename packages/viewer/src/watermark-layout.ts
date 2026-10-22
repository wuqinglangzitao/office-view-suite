export interface WatermarkTile {
  readonly url: string;
  readonly width: number;
  readonly height: number;
}

export interface WatermarkBounds {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** Read only document boxes: an old overlay must never become the next measurement. */
export function measureWatermarkBounds(
  viewport: HTMLElement,
  surface: HTMLElement,
  content: HTMLElement,
): WatermarkBounds {
  const outer = viewport.getBoundingClientRect();
  const box = surface.getBoundingClientRect();
  const documentBox = content.getBoundingClientRect();
  if (box.width <= 0 || box.height <= 0) return { left: 0, top: 0, width: 0, height: 0 };

  // scrollWidth/Height are local CSS pixels. CSS zoom applies to content only;
  // the surface's scroll range already includes that visual scaling.
  const zoom = Number(surface.ownerDocument.defaultView?.getComputedStyle(content).zoom) || 1;
  const contentLeft = documentBox.left - box.left;
  const contentTop = documentBox.top - box.top;
  return {
    left: Math.max(0, box.left - outer.left + viewport.scrollLeft - viewport.clientLeft),
    top: Math.max(0, box.top - outer.top + viewport.scrollTop - viewport.clientTop),
    width: Math.max(box.width, surface.scrollWidth,
      contentLeft + documentBox.width, contentLeft + content.scrollWidth * zoom),
    height: Math.max(box.height, surface.scrollHeight,
      contentTop + documentBox.height, contentTop + content.scrollHeight * zoom),
  };
}

type SavedStyle = { element: HTMLElement; property: string; value: string; priority: string };

/** A single, unscaled, non-interactive layer owned by the facade, outside renderer DOM. */
export function createWatermarkLayout(viewport: HTMLElement, surface: HTMLElement, content: HTMLElement) {
  const document = viewport.ownerDocument;
  const window = document.defaultView!;
  let overlay: HTMLDivElement | undefined;
  let pattern: HTMLDivElement | undefined;
  let observer: ResizeObserver | undefined;
  let mutations: MutationObserver | undefined;
  let frame: number | undefined;
  let previous: WatermarkBounds | undefined;
  let styles: SavedStyle[] = [];
  let destroyed = false;

  const request = (): void => {
    if (!overlay || destroyed || frame !== undefined) return;
    frame = window.requestAnimationFrame(() => {
      frame = undefined;
      if (!overlay) return;
      const bounds = measureWatermarkBounds(viewport, surface, content);
      if (previous && (Object.keys(bounds) as Array<keyof WatermarkBounds>).every(key => bounds[key] === previous![key])) return;
      previous = bounds;
      overlay.style.display = bounds.width > 0 && bounds.height > 0 ? 'block' : 'none';
      overlay.style.left = `${bounds.left}px`;
      overlay.style.top = `${bounds.top}px`;
      overlay.style.width = `${bounds.width}px`;
      overlay.style.height = `${bounds.height}px`;
    });
  };

  const setStyle = (element: HTMLElement, property: string, value: string): void => {
    styles.push({ element, property, value: element.style.getPropertyValue(property), priority: element.style.getPropertyPriority(property) });
    element.style.setProperty(property, value);
  };

  const hide = (): void => {
    if (!overlay) return;
    observer?.disconnect();
    observer = undefined;
    mutations?.disconnect();
    mutations = undefined;
    content.removeEventListener('load', request, true);
    window.removeEventListener('resize', request);
    if (frame !== undefined) window.cancelAnimationFrame(frame);
    frame = undefined;
    overlay?.remove();
    overlay = undefined;
    pattern = undefined;
    previous = undefined;
    for (const { element, property, value, priority } of styles) {
      if (value) element.style.setProperty(property, value, priority);
      else element.style.removeProperty(property);
    }
    styles = [];
  };

  return {
    show(tile: WatermarkTile): void {
      if (destroyed) return;
      if (!overlay) {
        // isolation creates a stacking context without changing the containing
        // block of any absolutely positioned renderer descendants.
        setStyle(viewport, 'position', 'relative');
        setStyle(viewport, 'isolation', 'isolate');
        setStyle(surface, 'isolation', 'isolate');
        overlay = document.createElement('div');
        overlay.className = 'office-viewer-watermark';
        overlay.setAttribute('aria-hidden', 'true');
        overlay.style.cssText = 'position:absolute;z-index:1;pointer-events:none;user-select:none;overflow:hidden;display:none;';
        const shadow = overlay.attachShadow({ mode: 'open' });
        pattern = document.createElement('div');
        pattern.style.cssText = 'width:100%;height:100%;pointer-events:none;user-select:none;background-repeat:repeat;';
        shadow.appendChild(pattern);
        viewport.appendChild(overlay);
        if (window.ResizeObserver) {
          observer = new window.ResizeObserver(request);
          observer.observe(surface);
          observer.observe(content);
        }
        if (window.MutationObserver) {
          mutations = new window.MutationObserver(request);
          mutations.observe(content, { subtree: true, childList: true, characterData: true, attributes: true,
            attributeFilter: ['style', 'class', 'width', 'height', 'hidden'] });
        }
        content.addEventListener('load', request, true);
        window.addEventListener('resize', request);
      }
      pattern!.style.backgroundImage = `url("${tile.url}")`;
      pattern!.style.backgroundSize = `${tile.width}px ${tile.height}px`;
      request();
    },
    hide,
    request,
    destroy(): void {
      if (destroyed) return;
      hide();
      destroyed = true;
    },
  };
}
