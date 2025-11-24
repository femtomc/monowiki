/**
 * Internal Link Previews
 * Shows preview of essays/thoughts when hovering over internal links
 */

import { resolveWithBase, stripBasePath } from './site-context';

interface PreviewData {
  title: string;
  preview: string;
  type: 'essay' | 'thought' | 'doc';
  has_toc?: boolean;
}

interface PreviewsMap {
  [url: string]: PreviewData;
}

interface PreviewConfig {
  hoverDelay: number;
  fadeoutDelay: number;
  maxWidth: number;
  offsetX: number;
  offsetY: number;
}

class LinkPreviewManager {
  private config: PreviewConfig = {
    hoverDelay: 500,
    fadeoutDelay: 100,
    maxWidth: 400,
    offsetX: 12,
    offsetY: 12,
  };

  private previews: PreviewsMap = {};
  private popup: HTMLDivElement | null = null;
  private showTimer: number | null = null;
  private hideTimer: number | null = null;
  private previewsLoaded = false;
  private previewsPromise: Promise<void> | null = null;

  async init(): Promise<void> {
    this.popup = document.createElement('div');
    this.popup.id = 'link-preview';
    this.popup.style.display = 'none';
    document.body.appendChild(this.popup);

    // Keep popup open when hovering over it
    this.popup.addEventListener('mouseenter', () => {
      if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    });

    this.popup.addEventListener('mouseleave', () => {
      this.hidePreview();
    });

    // Attach hover listeners to all internal links
    this.attachListeners();
  }

  private attachListeners(): void {
    document.querySelectorAll<HTMLAnchorElement>('a[href$=".html"]').forEach(link => {
      const href = link.getAttribute('href');
      if (!href) return;
      const urlKey = this.normalizeHref(href);
      if (!urlKey) return;

      link.addEventListener('mouseenter', (e) => this.onLinkHover(e, urlKey, link));
      link.addEventListener('mouseleave', () => this.onLinkLeave());
    });
  }

  private onLinkHover(_e: MouseEvent, urlKey: string, linkElement: HTMLAnchorElement): void {
    // Clear any existing timers
    if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    if (this.showTimer !== null) clearTimeout(this.showTimer);

    // Set timer to show popup after delay
    this.showTimer = window.setTimeout(async () => {
      const ok = await this.ensurePreviewsLoaded();
      if (!ok) return;
      if (!this.previews[urlKey]) return;
      this.showPreview(urlKey, linkElement);
    }, this.config.hoverDelay);
  }

  private onLinkLeave(): void {
    // Clear show timer
    if (this.showTimer !== null) clearTimeout(this.showTimer);

    // Hide after delay (gives time to move mouse to popup)
    this.hideTimer = window.setTimeout(() => {
      this.hidePreview();
    }, 200); // Increased from fadeoutDelay to give time to reach popup
  }

  private showPreview(urlKey: string, linkElement: HTMLAnchorElement): void {
    const data = this.previews[urlKey];
    if (!data || !this.popup) return;

    // Build popup HTML
    const typeLabel = data.type === 'thought' ? 'Thought' :
                      data.type === 'doc' ? 'Doc' : 'Essay';

    // If preview contains TOC HTML, fix relative anchor links to point to target page
    let previewContent: string;
    if (data.has_toc) {
      // Replace relative anchor hrefs (#section) with absolute links (/page.html#section)
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

    // Show popup (invisible) so we can measure its dimensions
    this.popup.style.visibility = 'hidden';
    this.popup.style.display = 'block';
    this.popup.style.opacity = '0';

    // Position after a frame so dimensions are calculated
    requestAnimationFrame(() => {
      if (!this.popup) return;
      this.positionPopupNearLink(linkElement);
      this.popup.style.visibility = 'visible';

      // Fade in
      requestAnimationFrame(() => {
        if (this.popup) {
          this.popup.style.opacity = '1';
        }
      });
    });
  }

  private hidePreview(): void {
    if (!this.popup) return;

    this.popup.style.opacity = '0';
    setTimeout(() => {
      if (this.popup) {
        this.popup.style.display = 'none';
      }
    }, 200);
  }

  private positionPopupNearLink(linkElement: HTMLAnchorElement): void {
    if (!this.popup) return;

    const linkRect = linkElement.getBoundingClientRect();
    const popupRect = this.popup.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    // Try to position below and to the right of the link
    let x = linkRect.left;
    let y = linkRect.bottom + 8;

    // If popup goes off-screen to the right, align to right edge of link
    if (x + popupRect.width > viewportWidth - 20) {
      x = Math.max(20, viewportWidth - popupRect.width - 20);
    }

    // If popup goes off-screen at bottom, position above the link
    if (y + popupRect.height > viewportHeight - 20) {
      y = linkRect.top - popupRect.height - 8;
    }

    // Ensure minimum margins
    x = Math.max(20, x);
    y = Math.max(20, y);

    this.popup.style.left = x + 'px';
    this.popup.style.top = y + 'px';
  }

  private escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  private normalizeHref(href: string): string | null {
    try {
      const targetUrl = resolveWithBase(href);
      return stripBasePath(targetUrl.pathname);
    } catch (e) {
      return null;
    }
  }

  private async ensurePreviewsLoaded(): Promise<boolean> {
    if (this.previewsLoaded) return true;
    if (!this.previewsPromise) {
      this.previewsPromise = (async () => {
        try {
          const response = await fetch(resolveWithBase('previews.json'));
          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }
          this.previews = await response.json();
          this.previewsLoaded = true;
          console.log('[+] Loaded', Object.keys(this.previews).length, 'link previews');
        } catch (e) {
          console.error('[-] Failed to load previews:', e);
        }
      })();
    }

    await this.previewsPromise;
    return this.previewsLoaded;
  }
}

// Singleton instance
let previewManager: LinkPreviewManager | null = null;

export function initPreviews(): void {
  if (!previewManager) {
    previewManager = new LinkPreviewManager();
    previewManager.init().catch(console.error);
  }
}

export type { PreviewData, PreviewsMap };
