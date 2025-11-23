import { beforeEach, describe, expect, it, vi } from 'vitest';

// These tests validate base-path handling for subpath deployments (e.g., /blog/).
describe('site-context', () => {
  beforeEach(() => {
    vi.resetModules();
    // Stub minimal window/document so site-context can derive base info
    (globalThis as any).window = { location: { origin: 'https://example.com' } };
    (globalThis as any).document = {
      baseURI: 'https://example.com/blog/post.html',
      querySelector: () => null,
    };
  });

  it('resolves relative paths with the base path', async () => {
    const ctx = await import('./site-context');
    const url = ctx.resolveWithBase('graph.json').toString();
    expect(url).toBe('https://example.com/blog/graph.json');
  });

  it('strips base path from pathnames', async () => {
    const ctx = await import('./site-context');
    expect(ctx.stripBasePath('/blog/page.html')).toBe('page.html');
    expect(ctx.stripBasePath('/blog/nested/page.html')).toBe('nested/page.html');
  });

  it('passes through absolute URLs untouched', async () => {
    const ctx = await import('./site-context');
    const url = ctx.resolveWithBase('https://other.site/x.html').toString();
    expect(url).toBe('https://other.site/x.html');
  });
});
