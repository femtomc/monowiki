/**
 * Preview pane management - iframe refresh and navigation.
 */

export interface PreviewOptions {
  iframe: HTMLIFrameElement;
  baseUrl: string;
}

export class Preview {
  private iframe: HTMLIFrameElement;
  private baseUrl: string;
  private currentSlug: string | null = null;

  constructor(options: PreviewOptions) {
    this.iframe = options.iframe;
    this.baseUrl = options.baseUrl;
  }

  setBaseUrl(url: string) {
    this.baseUrl = url;
    if (this.currentSlug) {
      this.navigate(this.currentSlug);
    }
  }

  /**
   * Navigate to a specific note in the preview.
   */
  navigate(slug: string) {
    this.currentSlug = slug;
    const url = this.buildUrl(slug);
    this.iframe.src = url;
  }

  /**
   * Refresh the current preview while preserving scroll position.
   */
  refresh() {
    // Try to get current scroll position
    let scrollX = 0;
    let scrollY = 0;
    try {
      const win = this.iframe.contentWindow;
      if (win) {
        scrollX = win.scrollX || 0;
        scrollY = win.scrollY || 0;
      }
    } catch {
      // Cross-origin or other error, ignore
    }

    // Use contentWindow.location.reload() to preserve more state
    try {
      const win = this.iframe.contentWindow;
      if (win && win.location.href !== 'about:blank') {
        // Reload the iframe content
        win.location.reload();

        // Restore scroll position after reload
        this.iframe.addEventListener('load', () => {
          try {
            const newWin = this.iframe.contentWindow;
            if (newWin) {
              newWin.scrollTo(scrollX, scrollY);
            }
          } catch {
            // Ignore cross-origin errors
          }
        }, { once: true });
        return;
      }
    } catch {
      // Fall back to src manipulation
    }

    // Fallback: force reload by appending a cache-busting query param
    if (this.iframe.src && this.iframe.src !== 'about:blank') {
      const url = new URL(this.iframe.src);
      url.searchParams.set('_t', Date.now().toString());

      // Store scroll position in iframe load handler
      this.iframe.addEventListener('load', () => {
        try {
          const win = this.iframe.contentWindow;
          if (win) {
            win.scrollTo(scrollX, scrollY);
          }
        } catch {
          // Ignore cross-origin errors
        }
      }, { once: true });

      this.iframe.src = url.toString();
    } else if (this.currentSlug) {
      this.navigate(this.currentSlug);
    }
  }

  /**
   * Build the full URL for a note slug.
   * Monowiki flattens output - vault/essays/foo.md becomes docs/foo.html
   */
  private buildUrl(slug: string): string {
    // Remove .md extension if present
    let cleanSlug = slug.replace(/\.md$/, '');
    // Extract just the filename (monowiki flattens directory structure)
    const lastSlash = cleanSlug.lastIndexOf('/');
    if (lastSlash !== -1) {
      cleanSlug = cleanSlug.substring(lastSlash + 1);
    }
    // Build URL - preview is served at /preview/{slug}.html
    const base = this.baseUrl.replace(/\/$/, '');
    return `${base}/${cleanSlug}.html`;
  }
}
