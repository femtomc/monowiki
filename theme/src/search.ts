/**
 * Search Component
 * Fuzzy search using MiniSearch
 */

import MiniSearch from 'minisearch';
import { resolveWithBase } from './site-context';

interface SearchDocument {
  id: string;
  url: string;
  title: string;
  section_title: string;
  content: string;
  snippet: string;
  tags: string[];
  type: 'essay' | 'thought' | 'doc';
}

class SearchManager {
  private searchIndex: MiniSearch<SearchDocument> | null = null;
  private documents: SearchDocument[] = [];
  private searchModal: HTMLElement | null = null;
  private searchInput: HTMLInputElement | null = null;
  private searchResults: HTMLDivElement | null = null;
  private selectedIndex: number = -1;
  private searchGraphCleanup: (() => void) | null = null;
  private indexLoadPromise: Promise<void> | null = null;
  private indexReady = false;
  private initialized = false;

  async init(): Promise<void> {
    if (this.initialized) return;
    this.initialized = true;

    // Set up UI first (must happen after DOM is ready)
    this.setupUI();
  }

  private setupUI(): void {
    this.searchModal = document.getElementById('search-modal');
    this.searchInput = document.getElementById('search-modal-input') as HTMLInputElement;
    this.searchResults = document.getElementById('search-modal-results') as HTMLDivElement;

    if (!this.searchModal || !this.searchInput || !this.searchResults) {
      console.warn('[~] Search modal elements not found');
      return;
    }

    // Search trigger button
    const searchTrigger = document.getElementById('search-trigger');
    if (searchTrigger) {
      searchTrigger.addEventListener('click', () => {
        this.open();
      });
    }

    // Tab switcher
    this.setupTabs();

    // Keyboard shortcut (Cmd/Ctrl + K) to open modal
    document.addEventListener('keydown', (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        this.open();
      }
      // Escape to close
      if (e.key === 'Escape' && this.searchModal?.classList.contains('active')) {
        this.close();
      }
      // Arrow key navigation
      if (this.searchModal?.classList.contains('active')) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          this.navigateResults(1);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          this.navigateResults(-1);
        } else if (e.key === 'Enter' && this.selectedIndex >= 0) {
          e.preventDefault();
          this.selectResult();
        }
      }
    });

    // Search on input
    this.searchInput.addEventListener('input', (e) => {
      const query = (e.target as HTMLInputElement).value;
      this.selectedIndex = -1;
      void this.performSearch(query);

      // Update graph if graph tab is active
      const graphTab = document.querySelector('.search-tab[data-tab="graph"]');
      if (graphTab?.classList.contains('active')) {
        void this.renderSearchGraph(query);
      }
    });

    // Close on backdrop click
    this.searchModal.addEventListener('click', (e) => {
      if (e.target === this.searchModal) {
        this.close();
      }
    });
  }

  private performSearch(query: string): void {
    if (!this.searchIndex || !this.searchResults) return;

    if (query.length < 2) {
      this.searchResults.innerHTML = '';
      return;
    }

    const results = this.searchIndex.search(query, {
      boost: { title: 3, section_title: 2, tags: 2 },
      fuzzy: 0.2,
      prefix: true,
    }).slice(0, 10);

    if (results.length === 0) {
      this.searchResults.innerHTML = '<div class="search-no-results">No results found</div>';
      this.searchResults.style.display = 'block';
      return;
    }

    // Render results
    const resultsList = document.createElement('ul');
    resultsList.className = 'search-results-list';

    results.forEach(result => {
      const li = document.createElement('li');
      const link = document.createElement('a');
      link.href = result.url;

      const sectionInfo = result.section_title
        ? `<span class="search-result-section">${this.escapeHtml(result.section_title)}</span>`
        : '';

      link.innerHTML = `
        <div class="search-result-header">
          <span class="search-result-type">${result.type}</span>
          <span class="search-result-title">${this.escapeHtml(result.title)}</span>
          ${sectionInfo}
        </div>
        <div class="search-result-snippet">${this.escapeHtml(result.snippet)}</div>
      `;
      li.appendChild(link);
      resultsList.appendChild(li);
    });

    this.searchResults.innerHTML = '';
    this.searchResults.appendChild(resultsList);
    this.searchResults.style.display = 'block';
  }

  open(): void {
    this.searchModal?.classList.add('active');
    this.searchInput?.focus();
    void this.ensureIndexLoaded();
  }

  close(): void {
    this.searchModal?.classList.remove('active');
    if (this.searchInput) this.searchInput.value = '';
    if (this.searchResults) this.searchResults.innerHTML = '';
    this.selectedIndex = -1;

    // Clean up graph if it was rendered
    if (this.searchGraphCleanup) {
      this.searchGraphCleanup();
      this.searchGraphCleanup = null;
    }

    // Reset to results tab
    const tabs = document.querySelectorAll('.search-tab');
    tabs.forEach((t) => t.classList.remove('active'));
    const resultsTab = document.querySelector('.search-tab[data-tab="results"]');
    resultsTab?.classList.add('active');

    const panels = document.querySelectorAll('.search-tab-panel');
    panels.forEach((p) => p.classList.remove('active'));
    document.getElementById('search-tab-results')?.classList.add('active');
  }

  private navigateResults(direction: number): void {
    const results = this.searchResults?.querySelectorAll('li');
    if (!results || results.length === 0) return;

    // Remove previous selection
    if (this.selectedIndex >= 0 && this.selectedIndex < results.length) {
      results[this.selectedIndex].classList.remove('selected');
    }

    // Update index
    this.selectedIndex += direction;
    if (this.selectedIndex < 0) this.selectedIndex = results.length - 1;
    if (this.selectedIndex >= results.length) this.selectedIndex = 0;

    // Add new selection
    results[this.selectedIndex].classList.add('selected');
    results[this.selectedIndex].scrollIntoView({ block: 'nearest' });
  }

  private selectResult(): void {
    const results = this.searchResults?.querySelectorAll('li a');
    if (!results || this.selectedIndex < 0 || this.selectedIndex >= results.length) return;

    const link = results[this.selectedIndex] as HTMLAnchorElement;
    window.location.href = link.href;
  }

  private setupTabs(): void {
    const tabs = document.querySelectorAll('.search-tab');
    tabs.forEach((tab) => {
      tab.addEventListener('click', () => {
        const tabName = tab.getAttribute('data-tab');
        if (!tabName) return;

        // Update tab buttons
        tabs.forEach((t) => t.classList.remove('active'));
        tab.classList.add('active');

        // Update tab panels
        const panels = document.querySelectorAll('.search-tab-panel');
        panels.forEach((p) => p.classList.remove('active'));

        const targetPanel = document.getElementById(`search-tab-${tabName}`);
        if (targetPanel) {
          targetPanel.classList.add('active');

          // Render filtered graph when switching to graph tab
          if (tabName === 'graph') {
            const query = this.searchInput?.value || '';
            void this.renderSearchGraph(query);
          }
        }
      });
    });
  }

  private async renderSearchGraph(query: string): Promise<void> {
    const container = document.getElementById('search-graph-container');
    if (!container) return;

    const ready = await this.ensureIndexLoaded();
    if (!ready) return;

    // Clean up previous graph
    if (this.searchGraphCleanup) {
      this.searchGraphCleanup();
      this.searchGraphCleanup = null;
    }

    try {
      // Import graph rendering
      const { renderGraph } = await import('./graph-visual');

      // Load graph data
      const response = await fetch(resolveWithBase('graph.json'));
      if (!response.ok) return;

      const graphData = await response.json();

      // Filter graph based on search query
      let filteredGraphData = graphData;
      if (query.length >= 2 && this.searchIndex) {
        // Get matching document IDs from search
        const searchResults = this.searchIndex.search(query, {
          boost: { title: 3, section_title: 2, tags: 2 },
          fuzzy: 0.2,
          prefix: true,
        });

        // Extract matching page slugs from search results
        const matchingSlugs = new Set<string>();
        searchResults.forEach((result) => {
          const doc = this.documents[parseInt(result.id)];
          if (doc) {
            // Extract slug from URL (/page.html or /page.html#section)
            const urlMatch = doc.url.match(/\/([^/]+)\.html/);
            if (urlMatch) {
              matchingSlugs.add(urlMatch[1]);
            }
          }
        });

        // Filter nodes and edges to show only matching pages and their connections
        if (matchingSlugs.size > 0) {
          const nodes = graphData.nodes.filter((n: any) => matchingSlugs.has(n.id));
          const nodeIds = new Set(nodes.map((n: any) => n.id));
          const edges = graphData.edges.filter(
            (e: any) => nodeIds.has(e.source) && nodeIds.has(e.target),
          );

          filteredGraphData = { nodes, edges };
          console.log(
            `[+] Filtered graph to ${nodes.length} matching nodes from search query: "${query}"`,
          );
        }
      }

      // Get current page slug for highlighting
      const path = window.location.pathname;
      const match = path.match(/\/([^/]+)\.html$/);
      const currentSlug = match ? match[1] : '';

      // Render filtered graph
      const config = {
        drag: true,
        zoom: true,
        depth: -1,
        scale: 1.0,
        repelForce: 0.5,
        centerForce: 0.3,
        linkDistance: 40,
        fontSize: 0.7,
        opacityScale: 1,
        focusOnHover: true,
      };

      this.searchGraphCleanup = await renderGraph(
        container,
        currentSlug,
        filteredGraphData,
        config,
      );
      console.log('[+] Graph rendered in search modal');
    } catch (e) {
      console.error('[!] Failed to render graph in search modal:', e);
    }
  }

  private escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  private async ensureIndexLoaded(): Promise<boolean> {
    if (this.indexReady) return true;
    if (!this.indexLoadPromise) {
      this.indexLoadPromise = (async () => {
        try {
          const response = await fetch(resolveWithBase('index.json'));
          if (!response.ok) {
            console.warn('[~] Search index not available');
            return;
          }
          this.documents = await response.json();

          this.searchIndex = new MiniSearch({
            fields: ['title', 'section_title', 'content', 'tags'],
            storeFields: ['url', 'title', 'section_title', 'snippet', 'type'],
          });

          this.searchIndex.addAll(this.documents.map((doc, idx) => ({
            ...doc,
            id: String(idx),
          })));

          this.indexReady = true;
          console.log('[+] Search ready - press Cmd/Ctrl+K to search');
        } catch (e) {
          console.warn('[~] Failed to load search index:', e);
        }
      })();
    }

    await this.indexLoadPromise;
    return this.indexReady;
  }
}

// Singleton instance
let searchManager: SearchManager | null = null;

export async function initSearch(): Promise<void> {
  if (!searchManager) {
    searchManager = new SearchManager();
  }
  await searchManager.init();
}

export function openSearchModal(): void {
  if (!searchManager) {
    searchManager = new SearchManager();
    searchManager.init().catch(console.error);
  }
  searchManager.open();
}

export type { SearchDocument };
