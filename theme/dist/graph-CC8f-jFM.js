var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
import { r as resolveWithBase, c as currentNoteSlug } from "./site-context-Qsxygl3d.js";
const defaultLocalConfig = {
  drag: true,
  zoom: true,
  depth: 1,
  scale: 1.1,
  repelForce: 0.5,
  centerForce: 0.3,
  linkDistance: 30,
  fontSize: 0.6,
  opacityScale: 1,
  focusOnHover: false
};
const defaultGlobalConfig = {
  drag: true,
  zoom: true,
  depth: -1,
  scale: 0.9,
  repelForce: 0.5,
  centerForce: 0.2,
  linkDistance: 30,
  fontSize: 0.6,
  opacityScale: 1,
  focusOnHover: true
};
class GraphManager {
  constructor() {
    __publicField(this, "graphLib", null);
    __publicField(this, "graphData", null);
    __publicField(this, "localCleanup", null);
    __publicField(this, "globalCleanup", null);
    __publicField(this, "initPromise", null);
  }
  async init() {
    if (!this.initPromise) {
      this.initPromise = this.initialize();
    }
    return this.initPromise;
  }
  async initialize() {
    try {
      const response = await fetch(resolveWithBase("graph.json"));
      if (!response.ok) {
        console.warn("[~] Graph data not available");
        return;
      }
      this.graphData = await response.json();
      if (this.graphData) {
        console.log("[+] Loaded graph with", this.graphData.nodes.length, "nodes");
      }
    } catch (e) {
      console.warn("[~] Failed to load graph:", e);
      return;
    }
    this.renderBacklinks();
    await this.initLocalGraph();
    this.setupGlobalGraphToggle();
    this.setupResizeHandler();
  }
  async initLocalGraph() {
    if (!this.graphData) return;
    const container = document.getElementById("graph-container");
    if (!container) return;
    const currentSlug = this.getCurrentSlug();
    if (!currentSlug) return;
    const { addToVisited } = await this.loadGraphLib();
    addToVisited(currentSlug);
    try {
      const { renderGraph } = await this.loadGraphLib();
      this.localCleanup = await renderGraph(
        container,
        currentSlug,
        this.graphData,
        defaultLocalConfig
      );
    } catch (e) {
      console.error("[!] Failed to render local graph:", e);
    }
  }
  setupGlobalGraphToggle() {
    const button = document.getElementById("global-graph-toggle");
    const modal = document.getElementById("global-graph-outer");
    if (!button || !modal) return;
    button.addEventListener("click", async () => {
      await this.showGlobalGraph();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && modal.classList.contains("active")) {
        this.hideGlobalGraph();
      }
    });
    modal.addEventListener("click", (e) => {
      if (e.target === modal) {
        this.hideGlobalGraph();
      }
    });
  }
  async renderGlobalGraph() {
    if (!this.graphData) return;
    const container = document.getElementById("global-graph-container");
    if (!container) return;
    const currentSlug = this.getCurrentSlug() || "";
    try {
      const { renderGraph } = await this.loadGraphLib();
      this.globalCleanup = await renderGraph(
        container,
        currentSlug,
        this.graphData,
        defaultGlobalConfig
      );
    } catch (e) {
      console.error("[!] Failed to render global graph:", e);
    }
  }
  hideGlobalGraph() {
    const modal = document.getElementById("global-graph-outer");
    modal == null ? void 0 : modal.classList.remove("active");
    if (this.globalCleanup) {
      this.globalCleanup();
      this.globalCleanup = null;
    }
    const container = document.getElementById("global-graph-container");
    if (container) {
      container.innerHTML = "";
    }
  }
  async showGlobalGraph() {
    await this.init();
    const modal = document.getElementById("global-graph-outer");
    if (!modal) return;
    modal.classList.add("active");
    await this.renderGlobalGraph();
  }
  setupResizeHandler() {
    let resizeTimeout;
    window.addEventListener("resize", () => {
      clearTimeout(resizeTimeout);
      resizeTimeout = window.setTimeout(async () => {
        const container = document.getElementById("graph-container");
        if (container && container.offsetWidth > 0 && container.offsetHeight > 0) {
          if (this.localCleanup) {
            this.localCleanup();
            this.localCleanup = null;
          }
          await this.initLocalGraph();
        }
      }, 100);
    });
  }
  cleanup() {
    if (this.localCleanup) {
      this.localCleanup();
      this.localCleanup = null;
    }
    if (this.globalCleanup) {
      this.globalCleanup();
      this.globalCleanup = null;
    }
  }
  renderBacklinks() {
    const container = document.getElementById("backlinks");
    if (!container || !this.graphData) return;
    const existingList = container.querySelector(".backlinks-list");
    if (existingList) {
      return;
    }
    const currentSlug = this.getCurrentSlug();
    if (!currentSlug) return;
    const backlinks = this.graphData.edges.filter((edge) => edge.target === currentSlug).map((edge) => this.graphData.nodes.find((n) => n.id === edge.source)).filter((node) => node !== void 0);
    if (backlinks.length === 0) {
      return;
    }
    const list = document.createElement("ul");
    list.className = "backlinks-list";
    backlinks.forEach((node) => {
      const li = document.createElement("li");
      const link = document.createElement("a");
      const targetPath = node.url || `${node.id}.html`;
      link.href = resolveWithBase(targetPath).toString();
      link.textContent = node.title;
      li.appendChild(link);
      list.appendChild(li);
    });
    container.appendChild(list);
  }
  getCurrentSlug() {
    return currentNoteSlug();
  }
  loadGraphLib() {
    if (!this.graphLib) {
      this.graphLib = import("./graph-visual-BjxlZS7t.js").then((n) => n.aq);
    }
    return this.graphLib;
  }
}
let graphManager = null;
async function initGraph() {
  if (!graphManager) {
    graphManager = new GraphManager();
  }
  await graphManager.init();
}
async function openGlobalGraph() {
  if (!graphManager) {
    graphManager = new GraphManager();
  }
  await graphManager.showGlobalGraph();
}
export {
  initGraph,
  openGlobalGraph
};
//# sourceMappingURL=graph-CC8f-jFM.js.map
