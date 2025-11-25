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
   * Refresh the current preview.
   */
  refresh() {
    if (this.iframe.src && this.iframe.src !== 'about:blank') {
      // Force reload by appending a cache-busting query param
      const url = new URL(this.iframe.src);
      url.searchParams.set('_t', Date.now().toString());
      this.iframe.src = url.toString();
    } else if (this.currentSlug) {
      this.navigate(this.currentSlug);
    }
  }

  /**
   * Build the full URL for a note slug.
   */
  private buildUrl(slug: string): string {
    // Remove .md extension if present
    const cleanSlug = slug.replace(/\.md$/, '');
    // Build URL - assuming dev server serves at /{slug}.html or /{slug}/
    const base = this.baseUrl.replace(/\/$/, '');
    return `${base}/${cleanSlug}.html`;
  }
}
