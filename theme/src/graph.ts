/**
 * Graph Visualization and Backlinks
 * Interactive force-directed graph based on Quartz
 */

import type { D3Config } from './graph-visual';
import { currentNoteSlug, resolveWithBase } from './site-context';

interface GraphNode {
  id: string;
  title: string;
  type: 'essay' | 'thought' | 'doc';
  url?: string;
  tags?: string[];
}

interface GraphEdge {
  source: string;
  target: string;
}

interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

const defaultLocalConfig: D3Config = {
  drag: true,
  zoom: true,
  depth: 1,
  scale: 1.1,
  repelForce: 0.5,
  centerForce: 0.3,
  linkDistance: 30,
  fontSize: 0.6,
  opacityScale: 1,
  focusOnHover: false,
};

const defaultGlobalConfig: D3Config = {
  drag: true,
  zoom: true,
  depth: -1,
  scale: 0.9,
  repelForce: 0.5,
  centerForce: 0.2,
  linkDistance: 30,
  fontSize: 0.6,
  opacityScale: 1,
  focusOnHover: true,
};

class GraphManager {
  private graphLib: Promise<typeof import('./graph-visual')> | null = null;
  private graphData: GraphData | null = null;
  private localCleanup: (() => void) | null = null;
  private globalCleanup: (() => void) | null = null;
  private initPromise: Promise<void> | null = null;

  async init(): Promise<void> {
    if (!this.initPromise) {
      this.initPromise = this.initialize();
    }
    return this.initPromise;
  }

  private async initialize(): Promise<void> {
    // Load graph data
    try {
      const response = await fetch(resolveWithBase('graph.json'));
      if (!response.ok) {
        console.warn('[~] Graph data not available');
        return;
      }
      this.graphData = await response.json();
      if (this.graphData) {
        console.log('[+] Loaded graph with', this.graphData.nodes.length, 'nodes');
      }
    } catch (e) {
      console.warn('[~] Failed to load graph:', e);
      return;
    }

    this.renderBacklinks();
    await this.initLocalGraph();
    this.setupGlobalGraphToggle();
    this.setupResizeHandler();
  }

  private async initLocalGraph(): Promise<void> {
    if (!this.graphData) return;

    const container = document.getElementById('graph-container');
    if (!container) return;

    const currentSlug = this.getCurrentSlug();
    if (!currentSlug) return;

    // Track visited page
    const { addToVisited } = await this.loadGraphLib();
    addToVisited(currentSlug);

    try {
      const { renderGraph } = await this.loadGraphLib();
      this.localCleanup = await renderGraph(
        container,
        currentSlug,
        this.graphData,
        defaultLocalConfig,
      );
    } catch (e) {
      console.error('[!] Failed to render local graph:', e);
    }
  }

  private setupGlobalGraphToggle(): void {
    const button = document.getElementById('global-graph-toggle');
    const modal = document.getElementById('global-graph-outer');

    if (!button || !modal) return;

    button.addEventListener('click', async () => {
      await this.showGlobalGraph();
    });

    // Close on escape
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && modal.classList.contains('active')) {
        this.hideGlobalGraph();
      }
    });

    // Close on backdrop click
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        this.hideGlobalGraph();
      }
    });
  }

  private async renderGlobalGraph(): Promise<void> {
    if (!this.graphData) return;

    const container = document.getElementById('global-graph-container');
    if (!container) return;

    const currentSlug = this.getCurrentSlug() || '';

    try {
      const { renderGraph } = await this.loadGraphLib();
      this.globalCleanup = await renderGraph(
        container,
        currentSlug,
        this.graphData,
        defaultGlobalConfig,
      );
    } catch (e) {
      console.error('[!] Failed to render global graph:', e);
    }
  }

  private hideGlobalGraph(): void {
    const modal = document.getElementById('global-graph-outer');
    modal?.classList.remove('active');

    if (this.globalCleanup) {
      this.globalCleanup();
      this.globalCleanup = null;
    }

    // Clean up rendered graph contents
    const container = document.getElementById('global-graph-container');
    if (container) {
      container.innerHTML = '';
    }
  }

  async showGlobalGraph(): Promise<void> {
    await this.init();
    const modal = document.getElementById('global-graph-outer');
    if (!modal) return;
    modal.classList.add('active');
    await this.renderGlobalGraph();
  }

  private setupResizeHandler(): void {
    let resizeTimeout: number | undefined;
    window.addEventListener('resize', () => {
      clearTimeout(resizeTimeout);
      resizeTimeout = window.setTimeout(async () => {
        // Re-render local graph on resize to handle viewport changes
        const container = document.getElementById('graph-container');
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

  cleanup(): void {
    if (this.localCleanup) {
      this.localCleanup();
      this.localCleanup = null;
    }
    if (this.globalCleanup) {
      this.globalCleanup();
      this.globalCleanup = null;
    }
  }

  private renderBacklinks(): void {
    const container = document.getElementById('backlinks');
    if (!container || !this.graphData) return;

    // Check if backlinks already exist (rendered by backend)
    const existingList = container.querySelector('.backlinks-list');
    if (existingList) {
      // Backlinks already rendered by backend, skip
      return;
    }

    const currentSlug = this.getCurrentSlug();
    if (!currentSlug) return;

    const backlinks = this.graphData.edges
      .filter((edge) => edge.target === currentSlug)
      .map((edge) => this.graphData!.nodes.find((n) => n.id === edge.source))
      .filter((node): node is GraphNode => node !== undefined);

    if (backlinks.length === 0) {
      return;
    }

    const list = document.createElement('ul');
    list.className = 'backlinks-list';

    backlinks.forEach((node) => {
      const li = document.createElement('li');
      const link = document.createElement('a');
      const targetPath = node.url || `${node.id}.html`;
      link.href = resolveWithBase(targetPath).toString();
      link.textContent = node.title;
      li.appendChild(link);
      list.appendChild(li);
    });

    container.appendChild(list);
  }

  private getCurrentSlug(): string | null {
    return currentNoteSlug();
  }

  private loadGraphLib() {
    if (!this.graphLib) {
      this.graphLib = import('./graph-visual');
    }
    return this.graphLib;
  }
}

let graphManager: GraphManager | null = null;

export async function initGraph(): Promise<void> {
  if (!graphManager) {
    graphManager = new GraphManager();
  }
  await graphManager.init();
}

export async function openGlobalGraph(): Promise<void> {
  if (!graphManager) {
    graphManager = new GraphManager();
  }
  await graphManager.showGlobalGraph();
}

export type { GraphData, GraphNode, GraphEdge };
