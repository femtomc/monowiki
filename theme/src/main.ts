/**
 * Monowiki Theme
 *
 * Main entry point for all frontend components
 */

import { initTOC } from './toc';
import { initMathCopy } from './math-copy';
import { initCopyPageSource } from './copy-page';
// import { initDarkMode } from './darkmode';

// Initialize all components when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
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
  // initDarkMode(); // Disabled for now
}

function setupPreviewsLoader() {
  let loaded = false;
  const load = async () => {
    if (loaded) return;
    loaded = true;
    try {
      const { initPreviews } = await import('./previews');
      initPreviews();
    } catch (err) {
      console.error('Failed to load previews', err);
    } finally {
      document.removeEventListener('pointerover', onPointerOver, true);
    }
  };

  const onPointerOver = (event: Event) => {
    const target = event.target;
    if (target instanceof HTMLAnchorElement) {
      const href = target.getAttribute('href') || '';
      if (href.endsWith('.html')) {
        load();
      }
    }
  };

  document.addEventListener('pointerover', onPointerOver, true);
}

function setupSearchLoader() {
  const trigger = document.getElementById('search-trigger');
  const modal = document.getElementById('search-modal');
  if (!modal) return;

  let loaded = false;
  let loadingPromise: Promise<void> | null = null;

  const loadSearch = async (openAfterLoad: boolean) => {
    if (!loadingPromise) {
      loadingPromise = (async () => {
        try {
          const { initSearch } = await import('./search');
          await initSearch();
          loaded = true;
        } catch (err) {
          console.error('Failed to load search', err);
        }
      })();
    }

    await loadingPromise;

    if (loaded && openAfterLoad) {
      const { openSearchModal } = await import('./search');
      openSearchModal();
    }
  };

  trigger?.addEventListener('click', () => {
    void loadSearch(true);
  });

  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      void loadSearch(true);
    }
  });
}

function setupGraphLoader() {
  const localGraph = document.getElementById('graph-container');
  const globalToggle = document.getElementById('global-graph-toggle');

  if (!localGraph && !globalToggle) {
    return;
  }

  let loaded = false;
  let loadingPromise: Promise<void> | null = null;

  const loadGraph = async (openAfterLoad: boolean) => {
    if (!loadingPromise) {
      loadingPromise = (async () => {
        try {
          const { initGraph } = await import('./graph');
          await initGraph();
          loaded = true;
        } catch (err) {
          console.error('Failed to load graph', err);
        }
      })();
    }

    await loadingPromise;

    if (loaded && openAfterLoad) {
      const { openGlobalGraph } = await import('./graph');
      await openGlobalGraph();
    }
  };

  // Load when user asks for global graph
  globalToggle?.addEventListener('click', () => {
    void loadGraph(true);
  });

  // Load when local graph enters viewport (or after idle fallback)
  if (localGraph) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          void loadGraph(false);
        }
      },
      { rootMargin: '200px' },
    );
    observer.observe(localGraph);

    // Fallback to idle load so graph is ready even if not scrolled
    if ('requestIdleCallback' in window) {
      (window as any).requestIdleCallback(() => void loadGraph(false));
    } else {
      setTimeout(() => void loadGraph(false), 1500);
    }
  }
}
