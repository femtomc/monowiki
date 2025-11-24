var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
class TOCManager {
  constructor() {
    __publicField(this, "tocContainer", null);
    __publicField(this, "tocItems", []);
    __publicField(this, "activeId", null);
  }
  init() {
    this.tocContainer = document.getElementById("toc");
    if (!this.tocContainer) return;
    const existingToc = this.tocContainer.querySelector(".toc-nav");
    if (existingToc) {
      this.extractHeadingsFromExistingTOC();
      this.setupScrollSpy();
      return;
    }
    this.extractHeadings();
    if (this.tocItems.length === 0) {
      this.tocContainer.style.display = "none";
      return;
    }
    this.renderTOC();
    this.setupScrollSpy();
  }
  extractHeadings() {
    const content = document.querySelector("article, main, .content");
    if (!content) return;
    const headings = content.querySelectorAll("h2, h3, h4");
    headings.forEach((heading) => {
      const h = heading;
      const level = parseInt(h.tagName.substring(1));
      if (!h.id) {
        h.id = this.slugify(h.textContent || "");
      }
      this.tocItems.push({
        id: h.id,
        text: h.textContent || "",
        level,
        element: h
      });
    });
  }
  extractHeadingsFromExistingTOC() {
    if (!this.tocContainer) return;
    const links = this.tocContainer.querySelectorAll(".toc-list a");
    links.forEach((link) => {
      const href = link.getAttribute("href");
      if (!href || !href.startsWith("#")) return;
      const id = href.substring(1);
      const heading = document.getElementById(id);
      if (!heading) return;
      const level = parseInt(heading.tagName.substring(1));
      const li = link.closest("li");
      if (li) {
        li.dataset.id = id;
      }
      this.tocItems.push({
        id,
        text: link.textContent || "",
        level,
        element: heading
      });
    });
  }
  renderTOC() {
    if (!this.tocContainer) return;
    const nav = document.createElement("nav");
    nav.className = "toc-nav";
    const title = document.createElement("h3");
    title.textContent = "Contents";
    nav.appendChild(title);
    const list = document.createElement("ul");
    list.className = "toc-list";
    this.tocItems.forEach((item) => {
      const li = document.createElement("li");
      li.className = `toc-level-${item.level}`;
      li.dataset.id = item.id;
      const link = document.createElement("a");
      link.href = `#${item.id}`;
      link.textContent = item.text;
      link.addEventListener("click", (e) => {
        e.preventDefault();
        this.scrollToHeading(item.id);
      });
      li.appendChild(link);
      list.appendChild(li);
    });
    nav.appendChild(list);
    this.tocContainer.appendChild(nav);
  }
  setupScrollSpy() {
    const options = {
      rootMargin: "-100px 0px -66%",
      threshold: 1
    };
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const id = entry.target.id;
          this.setActive(id);
        }
      });
    }, options);
    this.tocItems.forEach((item) => {
      observer.observe(item.element);
    });
  }
  setActive(id) {
    var _a, _b;
    if (this.activeId === id) return;
    if (this.activeId) {
      const prevItem = (_a = this.tocContainer) == null ? void 0 : _a.querySelector(`[data-id="${this.activeId}"]`);
      prevItem == null ? void 0 : prevItem.classList.remove("active");
    }
    this.activeId = id;
    const newItem = (_b = this.tocContainer) == null ? void 0 : _b.querySelector(`[data-id="${id}"]`);
    newItem == null ? void 0 : newItem.classList.add("active");
  }
  scrollToHeading(id) {
    const heading = document.getElementById(id);
    if (!heading) return;
    heading.scrollIntoView({ behavior: "smooth", block: "start" });
    window.history.pushState(null, "", `#${id}`);
  }
  slugify(text) {
    return text.toLowerCase().replace(/[^\w\s-]/g, "").replace(/\s+/g, "-").replace(/-+/g, "-").trim();
  }
}
let tocManager = null;
function initTOC() {
  if (!tocManager) {
    tocManager = new TOCManager();
    tocManager.init();
  }
}
function initMathCopy() {
  const mathNodes = /* @__PURE__ */ new Set();
  const attach = (mathEl) => {
    if (!(mathEl instanceof HTMLElement) || mathNodes.has(mathEl)) return;
    mathNodes.add(mathEl);
    mathEl.style.userSelect = "text";
    mathEl.style.cursor = "text";
    const handleCopy = (e) => {
      const mathSource = mathEl.getAttribute("data-math") || "";
      if (!mathSource || !selectionIntersects(mathEl)) return;
      e.preventDefault();
      copyText$2(mathSource).then();
    };
    mathEl.addEventListener("copy", handleCopy);
  };
  document.querySelectorAll(".typst-math").forEach(attach);
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => {
        var _a, _b;
        if (!(node instanceof Element)) return;
        if ((_a = node.classList) == null ? void 0 : _a.contains("typst-math")) {
          attach(node);
        }
        (_b = node.querySelectorAll) == null ? void 0 : _b.call(node, ".typst-math").forEach(attach);
      });
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
  document.addEventListener("selectionchange", () => {
    const sel = window.getSelection();
    mathNodes.forEach((el) => {
      if (sel && !sel.isCollapsed && selectionIntersects(el)) {
        el.classList.add("selecting");
      } else {
        el.classList.remove("selecting");
      }
    });
  });
}
async function copyText$2(text) {
  var _a;
  try {
    if ((_a = navigator.clipboard) == null ? void 0 : _a.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (err) {
    console.warn("Navigator clipboard copy failed, falling back", err);
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "absolute";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  let success = false;
  try {
    success = document.execCommand("copy");
  } catch (err) {
    console.warn("Fallback copy failed", err);
  } finally {
    document.body.removeChild(textarea);
  }
  return success;
}
function selectionIntersects(el) {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) return false;
  try {
    const range = sel.getRangeAt(0);
    return range.intersectsNode(el);
  } catch {
    return false;
  }
}
function initCopyPageSource() {
  const button = document.getElementById("copy-page-source");
  const source = readSourcePayload();
  if (!(button instanceof HTMLButtonElement)) return;
  const defaultLabel = button.textContent || "Copy page source";
  if (!source) {
    button.disabled = true;
    button.title = "Source unavailable";
    return;
  }
  button.addEventListener("click", async () => {
    const success = await copyText$1(source);
    showStatus$1(button, success ? "Copied!" : "Copy failed", defaultLabel);
  });
}
function readSourcePayload() {
  const el = document.getElementById("page-source-data");
  if (!el) return null;
  try {
    const raw = el.textContent || "";
    return JSON.parse(raw);
  } catch (err) {
    console.warn("Failed to parse page source payload", err);
    return null;
  }
}
async function copyText$1(text) {
  var _a;
  try {
    if ((_a = navigator.clipboard) == null ? void 0 : _a.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (err) {
    console.warn("Navigator clipboard copy failed, falling back", err);
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "absolute";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  let success = false;
  try {
    success = document.execCommand("copy");
  } catch (err) {
    console.warn("Fallback copy failed", err);
  } finally {
    document.body.removeChild(textarea);
  }
  return success;
}
function showStatus$1(button, status, defaultLabel) {
  const prev = button.textContent;
  button.textContent = status;
  button.dataset.state = status.toLowerCase().includes("copied") ? "copied" : "error";
  setTimeout(() => {
    button.textContent = prev || defaultLabel;
    button.dataset.state = "";
  }, 1200);
}
function initCopyCode() {
  document.querySelectorAll(".copy-code-btn").forEach((button) => {
    button.addEventListener("click", async () => {
      const codeBlock = button.closest(".code-block");
      const pre = codeBlock == null ? void 0 : codeBlock.querySelector("pre");
      if (!pre) return;
      const code = pre.textContent || "";
      const success = await copyText(code);
      showStatus(button, success);
    });
  });
}
async function copyText(text) {
  var _a;
  try {
    if ((_a = navigator.clipboard) == null ? void 0 : _a.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (err) {
    console.warn("Navigator clipboard copy failed, falling back", err);
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "absolute";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  let success = false;
  try {
    success = document.execCommand("copy");
  } catch (err) {
    console.warn("Fallback copy failed", err);
  } finally {
    document.body.removeChild(textarea);
  }
  return success;
}
function showStatus(button, success) {
  const originalText = button.textContent;
  button.textContent = success ? "Copied!" : "Failed";
  button.dataset.state = success ? "copied" : "error";
  setTimeout(() => {
    button.textContent = originalText;
    button.dataset.state = "";
  }, 1200);
}
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
function init() {
  setupPreviewsLoader();
  setupSearchLoader();
  setupGraphLoader();
  initTOC();
  initMathCopy();
  initCopyPageSource();
  initCopyCode();
}
function setupPreviewsLoader() {
  let loaded = false;
  const load = async () => {
    if (loaded) return;
    loaded = true;
    try {
      const { initPreviews } = await import("./previews-C0a62L67.js");
      initPreviews();
    } catch (err) {
      console.error("Failed to load previews", err);
    } finally {
      document.removeEventListener("pointerover", onPointerOver, true);
    }
  };
  const onPointerOver = (event) => {
    const target = event.target;
    if (target instanceof HTMLAnchorElement) {
      const href = target.getAttribute("href") || "";
      if (href.endsWith(".html")) {
        load();
      }
    }
  };
  document.addEventListener("pointerover", onPointerOver, true);
}
function setupSearchLoader() {
  const trigger = document.getElementById("search-trigger");
  const modal = document.getElementById("search-modal");
  if (!modal) return;
  let loaded = false;
  let loadingPromise = null;
  const loadSearch = async (openAfterLoad) => {
    if (!loadingPromise) {
      loadingPromise = (async () => {
        try {
          const { initSearch } = await import("./search-CEwqX6Pz.js");
          await initSearch();
          loaded = true;
        } catch (err) {
          console.error("Failed to load search", err);
        }
      })();
    }
    await loadingPromise;
    if (loaded && openAfterLoad) {
      const { openSearchModal } = await import("./search-CEwqX6Pz.js");
      openSearchModal();
    }
  };
  trigger == null ? void 0 : trigger.addEventListener("click", () => {
    void loadSearch(true);
  });
  document.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      void loadSearch(true);
    }
  });
}
function setupGraphLoader() {
  const localGraph = document.getElementById("graph-container");
  const globalToggle = document.getElementById("global-graph-toggle");
  if (!localGraph && !globalToggle) {
    return;
  }
  let loaded = false;
  let loadingPromise = null;
  const loadGraph = async (openAfterLoad) => {
    if (!loadingPromise) {
      loadingPromise = (async () => {
        try {
          const { initGraph } = await import("./graph-CC8f-jFM.js");
          await initGraph();
          loaded = true;
        } catch (err) {
          console.error("Failed to load graph", err);
        }
      })();
    }
    await loadingPromise;
    if (loaded && openAfterLoad) {
      const { openGlobalGraph } = await import("./graph-CC8f-jFM.js");
      await openGlobalGraph();
    }
  };
  globalToggle == null ? void 0 : globalToggle.addEventListener("click", () => {
    void loadGraph(true);
  });
  if (localGraph) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          void loadGraph(false);
        }
      },
      { rootMargin: "200px" }
    );
    observer.observe(localGraph);
    if ("requestIdleCallback" in window) {
      window.requestIdleCallback(() => void loadGraph(false));
    } else {
      setTimeout(() => void loadGraph(false), 1500);
    }
  }
}
//# sourceMappingURL=bundle.js.map
