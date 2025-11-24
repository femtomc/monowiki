const BASE_META = "monowiki-base-url";
const SLUG_META = "monowiki-note-slug";
function readMeta(name) {
  var _a;
  return ((_a = document.querySelector(`meta[name="${name}"]`)) == null ? void 0 : _a.content) ?? null;
}
function normalizeBasePath(raw) {
  if (!raw) return "/";
  let path = raw.trim();
  if (!path.startsWith("/")) {
    path = `/${path}`;
  }
  path = path.replace(/\/+$/, "/");
  path = path.replace(/\/{2,}/g, "/");
  return path || "/";
}
function deriveBasePath() {
  const urlObj = new URL(document.baseURI);
  const directory = urlObj.pathname.replace(/\/[^/]*$/, "/");
  return normalizeBasePath(directory);
}
const BASE_PATH = normalizeBasePath(readMeta(BASE_META) ?? deriveBasePath());
const BASE_URL = new URL(BASE_PATH, window.location.origin);
function getBasePath() {
  const path = BASE_URL.pathname;
  return path.endsWith("/") ? path : `${path}/`;
}
function resolveWithBase(path) {
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(path)) {
    return new URL(path);
  }
  const clean = path.replace(/^\/+/, "");
  const joined = `${getBasePath()}${clean}`;
  return new URL(joined, window.location.origin);
}
function stripBasePath(pathname) {
  const basePath = getBasePath();
  let path = pathname;
  if (path.startsWith(basePath)) {
    path = path.slice(basePath.length);
  }
  return path.replace(/^\/+/, "");
}
function currentNoteSlug() {
  var _a;
  const metaSlug = (_a = readMeta(SLUG_META)) == null ? void 0 : _a.trim();
  if (metaSlug) return metaSlug;
  const relativePath = stripBasePath(window.location.pathname);
  if (!relativePath) return null;
  const withoutIndex = relativePath.endsWith("index.html") ? relativePath.slice(0, -"index.html".length) : relativePath;
  const withoutHtml = withoutIndex.endsWith(".html") ? withoutIndex.slice(0, -".html".length) : withoutIndex;
  if (!withoutHtml) return null;
  const segments = withoutHtml.split("/").filter(Boolean);
  return segments.pop() ?? null;
}
export {
  currentNoteSlug as c,
  resolveWithBase as r,
  stripBasePath as s
};
//# sourceMappingURL=site-context-Qsxygl3d.js.map
