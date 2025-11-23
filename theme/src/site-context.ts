/**
 * Shared helpers for resolving monowiki base URLs and current page context.
 * The server injects meta tags so we don't have to guess from the current path.
 */

const BASE_META = 'monowiki-base-url';
const SLUG_META = 'monowiki-note-slug';

function readMeta(name: string): string | null {
  return document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`)?.content ?? null;
}

function normalizeBasePath(raw: string): string {
  if (!raw) return '/';
  let path = raw.trim();
  if (!path.startsWith('/')) {
    path = `/${path}`;
  }
  // Ensure a single trailing slash
  path = path.replace(/\/+$/, '/');
  // Collapse duplicate slashes (except the leading one we forced above)
  path = path.replace(/\/{2,}/g, '/');
  return path || '/';
}

function deriveBasePath(): string {
  const urlObj = new URL(document.baseURI);
  const directory = urlObj.pathname.replace(/\/[^/]*$/, '/');
  return normalizeBasePath(directory);
}

const BASE_PATH = normalizeBasePath(readMeta(BASE_META) ?? deriveBasePath());
const BASE_URL = new URL(BASE_PATH, window.location.origin);

/** Absolute base URL for the site (includes origin + base path). */
export function getBaseUrl(): string {
  return BASE_URL.toString();
}

/** Base path portion (always starts/ends with "/"). */
export function getBasePath(): string {
  const path = BASE_URL.pathname;
  return path.endsWith('/') ? path : `${path}/`;
}

/** Resolve a path against the monowiki base path (keeps subpaths intact). */
export function resolveWithBase(path: string): URL {
  // Absolute URL passthrough
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(path)) {
    return new URL(path);
  }

  const clean = path.replace(/^\/+/, '');
  const joined = `${getBasePath()}${clean}`;
  return new URL(joined, window.location.origin);
}

/** Strip the configured base path from a pathname and drop leading slashes. */
export function stripBasePath(pathname: string): string {
  const basePath = getBasePath();
  let path = pathname;
  if (path.startsWith(basePath)) {
    path = path.slice(basePath.length);
  }
  return path.replace(/^\/+/, '');
}

/** Best-effort current note slug (uses injected meta when available). */
export function currentNoteSlug(): string | null {
  const metaSlug = readMeta(SLUG_META)?.trim();
  if (metaSlug) return metaSlug;

  const relativePath = stripBasePath(window.location.pathname);
  if (!relativePath) return null;

  const withoutIndex = relativePath.endsWith('index.html')
    ? relativePath.slice(0, -'index.html'.length)
    : relativePath;
  const withoutHtml = withoutIndex.endsWith('.html')
    ? withoutIndex.slice(0, -'.html'.length)
    : withoutIndex;

  if (!withoutHtml) return null;

  const segments = withoutHtml.split('/').filter(Boolean);
  return segments.pop() ?? null;
}
