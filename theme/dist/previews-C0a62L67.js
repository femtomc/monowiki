var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
import { r as resolveWithBase, s as stripBasePath } from "./site-context-Qsxygl3d.js";
class LinkPreviewManager {
  constructor() {
    __publicField(this, "config", {
      hoverDelay: 500,
      fadeoutDelay: 100,
      maxWidth: 400,
      offsetX: 12,
      offsetY: 12
    });
    __publicField(this, "previews", {});
    __publicField(this, "popup", null);
    __publicField(this, "showTimer", null);
    __publicField(this, "hideTimer", null);
    __publicField(this, "previewsLoaded", false);
    __publicField(this, "previewsPromise", null);
  }
  async init() {
    this.popup = document.createElement("div");
    this.popup.id = "link-preview";
    this.popup.style.display = "none";
    document.body.appendChild(this.popup);
    this.popup.addEventListener("mouseenter", () => {
      if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    });
    this.popup.addEventListener("mouseleave", () => {
      this.hidePreview();
    });
    this.attachListeners();
  }
  attachListeners() {
    document.querySelectorAll('a[href$=".html"]').forEach((link) => {
      const href = link.getAttribute("href");
      if (!href) return;
      const urlKey = this.normalizeHref(href);
      if (!urlKey) return;
      link.addEventListener("mouseenter", (e) => this.onLinkHover(e, urlKey, link));
      link.addEventListener("mouseleave", () => this.onLinkLeave());
    });
  }
  onLinkHover(_e, urlKey, linkElement) {
    if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    if (this.showTimer !== null) clearTimeout(this.showTimer);
    this.showTimer = window.setTimeout(async () => {
      const ok = await this.ensurePreviewsLoaded();
      if (!ok) return;
      if (!this.previews[urlKey]) return;
      this.showPreview(urlKey, linkElement);
    }, this.config.hoverDelay);
  }
  onLinkLeave() {
    if (this.showTimer !== null) clearTimeout(this.showTimer);
    this.hideTimer = window.setTimeout(() => {
      this.hidePreview();
    }, 200);
  }
  showPreview(urlKey, linkElement) {
    const data = this.previews[urlKey];
    if (!data || !this.popup) return;
    const typeLabel = data.type === "thought" ? "Thought" : data.type === "doc" ? "Doc" : "Essay";
    let previewContent;
    if (data.has_toc) {
      const targetUrl = resolveWithBase(urlKey).pathname;
      previewContent = data.preview.replace(/href="#/g, `href="${targetUrl}#`);
    } else {
      previewContent = `<p>${this.escapeHtml(data.preview)}</p>`;
    }
    this.popup.innerHTML = `
      <div class="preview-header">
        <span class="preview-type">${typeLabel}</span>
        <span class="preview-title">${this.escapeHtml(data.title)}</span>
      </div>
      <div class="preview-content">
        ${previewContent}
      </div>
    `;
    this.popup.style.visibility = "hidden";
    this.popup.style.display = "block";
    this.popup.style.opacity = "0";
    requestAnimationFrame(() => {
      if (!this.popup) return;
      this.positionPopupNearLink(linkElement);
      this.popup.style.visibility = "visible";
      requestAnimationFrame(() => {
        if (this.popup) {
          this.popup.style.opacity = "1";
        }
      });
    });
  }
  hidePreview() {
    if (!this.popup) return;
    this.popup.style.opacity = "0";
    setTimeout(() => {
      if (this.popup) {
        this.popup.style.display = "none";
      }
    }, 200);
  }
  positionPopupNearLink(linkElement) {
    if (!this.popup) return;
    const linkRect = linkElement.getBoundingClientRect();
    const popupRect = this.popup.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    let x = linkRect.left;
    let y = linkRect.bottom + 8;
    if (x + popupRect.width > viewportWidth - 20) {
      x = Math.max(20, viewportWidth - popupRect.width - 20);
    }
    if (y + popupRect.height > viewportHeight - 20) {
      y = linkRect.top - popupRect.height - 8;
    }
    x = Math.max(20, x);
    y = Math.max(20, y);
    this.popup.style.left = x + "px";
    this.popup.style.top = y + "px";
  }
  escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }
  normalizeHref(href) {
    try {
      const targetUrl = resolveWithBase(href);
      return stripBasePath(targetUrl.pathname);
    } catch (e) {
      return null;
    }
  }
  async ensurePreviewsLoaded() {
    if (this.previewsLoaded) return true;
    if (!this.previewsPromise) {
      this.previewsPromise = (async () => {
        try {
          const response = await fetch(resolveWithBase("previews.json"));
          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }
          this.previews = await response.json();
          this.previewsLoaded = true;
          console.log("[+] Loaded", Object.keys(this.previews).length, "link previews");
        } catch (e) {
          console.error("[-] Failed to load previews:", e);
        }
      })();
    }
    await this.previewsPromise;
    return this.previewsLoaded;
  }
}
let previewManager = null;
function initPreviews() {
  if (!previewManager) {
    previewManager = new LinkPreviewManager();
    previewManager.init().catch(console.error);
  }
}
export {
  initPreviews
};
//# sourceMappingURL=previews-C0a62L67.js.map
