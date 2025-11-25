/**
 * monowiki-editor main entry point.
 *
 * Wires together:
 * - CodeMirror 6 editor with Yjs/y-websocket
 * - Split pane with resizer
 * - Preview iframe (points to dev server)
 * - Toolbar actions (open, checkpoint, build, refresh)
 */

import { createEditor, EditorInstance, ConnectionStatus } from './editor';
import { CollabAPI, FileEntry } from './api';
import { Preview } from './preview';

// DOM elements
const slugInput = document.getElementById('slug-input') as HTMLInputElement;
const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
const connectionStatus = document.getElementById('connection-status') as HTMLSpanElement;
const flushBtn = document.getElementById('flush-btn') as HTMLButtonElement;
const checkpointBtn = document.getElementById('checkpoint-btn') as HTMLButtonElement;
const buildBtn = document.getElementById('build-btn') as HTMLButtonElement;
const previewUrlInput = document.getElementById('preview-url') as HTMLInputElement;
const refreshBtn = document.getElementById('refresh-btn') as HTMLButtonElement;
const editorContainer = document.getElementById('editor') as HTMLDivElement;
const previewFrame = document.getElementById('preview-frame') as HTMLIFrameElement;
const resizer = document.getElementById('resizer') as HTMLDivElement;
const editorPane = document.querySelector('.editor-pane') as HTMLDivElement;
const previewPane = document.querySelector('.preview-pane') as HTMLDivElement;
const tokenInput = document.getElementById('token-input') as HTMLInputElement;

// Sidebar elements
const fileTree = document.getElementById('file-tree') as HTMLDivElement;
const newFileBtn = document.getElementById('new-file-btn') as HTMLButtonElement;
const sidebar = document.getElementById('sidebar') as HTMLElement;
const sidebarResizer = document.getElementById('sidebar-resizer') as HTMLDivElement;

// State
let currentEditor: EditorInstance | null = null;
let currentSlug: string | null = null;
let renderTimeout: number | null = null;
const RENDER_DEBOUNCE_MS = 100; // Wait 100ms after typing stops before rendering

// API client - in dev mode, Vite proxies /api and /ws to localhost:8787
const api = new CollabAPI('');

// Preview manager - defaults to /preview (served by collab daemon)
const preview = new Preview({
  iframe: previewFrame,
  baseUrl: previewUrlInput.value || '/preview',
});

// Load saved preferences
function loadPreferences() {
  const savedPreviewUrl = localStorage.getItem('monowiki-preview-url');
  if (savedPreviewUrl) {
    previewUrlInput.value = savedPreviewUrl;
    preview.setBaseUrl(savedPreviewUrl);
  }

  const savedSlug = localStorage.getItem('monowiki-last-slug');
  if (savedSlug) {
    slugInput.value = savedSlug;
  }

  const savedToken = localStorage.getItem('monowiki-token');
  if (savedToken) {
    api.setToken(savedToken);
    tokenInput.value = savedToken;
  }
}

function savePreferences() {
  localStorage.setItem('monowiki-preview-url', previewUrlInput.value);
  if (currentSlug) {
    localStorage.setItem('monowiki-last-slug', currentSlug);
  }
  if (tokenInput.value) {
    localStorage.setItem('monowiki-token', tokenInput.value);
  }
}

// Update connection status display
function updateStatusDisplay(status: ConnectionStatus) {
  connectionStatus.textContent = status;
  connectionStatus.className = `status ${status}`;
}

// Debounced incremental render - call this when content changes
function scheduleRender() {
  if (!currentSlug) return;

  // Clear any pending render
  if (renderTimeout !== null) {
    clearTimeout(renderTimeout);
  }

  // Schedule new render after debounce delay
  renderTimeout = window.setTimeout(async () => {
    if (!currentSlug) return;

    try {
      await api.render(currentSlug);
      // Refresh preview after render completes
      preview.refresh();
    } catch (err) {
      console.error('Incremental render failed:', err);
      // Don't show alert for render failures - they're not critical
    }
  }, RENDER_DEBOUNCE_MS);
}

// Open a note for editing
async function openNote(slug: string) {
  if (!slug.trim()) {
    alert('Please enter a note slug');
    return;
  }

  // Clean up existing editor
  if (currentEditor) {
    currentEditor.destroy();
    currentEditor = null;
  }

  currentSlug = slug;
  savePreferences();

  // Update status
  updateStatusDisplay('connecting');

  try {
    // Save token to API + local storage
    if (tokenInput.value) {
      api.setToken(tokenInput.value);
      savePreferences();
    }

    // Create new editor connected to the slug's WebSocket
    const wsBaseUrl = api.wsBaseUrl();
    currentEditor = createEditor({
      container: editorContainer,
      wsBaseUrl,
      token: tokenInput.value || undefined,
      room: slug,
      onStatusChange: updateStatusDisplay,
      onContentChange: scheduleRender,
    });

    // Navigate preview
    preview.navigate(slug);

  } catch (err) {
    console.error('Failed to open note:', err);
    updateStatusDisplay('disconnected');
    alert(`Failed to open note: ${err}`);
  }
}

// Checkpoint (commit and push)
async function doCheckpoint() {
  checkpointBtn.disabled = true;
  checkpointBtn.textContent = 'Checkpointing...';

  try {
    const result = await api.checkpoint();
    console.log('Checkpoint result:', result);
    alert(result.message);
  } catch (err) {
    console.error('Checkpoint failed:', err);
    alert(`Checkpoint failed: ${err}`);
  } finally {
    checkpointBtn.disabled = false;
    checkpointBtn.textContent = 'Checkpoint';
  }
}

// Build (and optionally deploy)
async function doBuild() {
  buildBtn.disabled = true;
  buildBtn.textContent = 'Building...';

  try {
    const result = await api.build();
    console.log('Build result:', result);
    alert('Build completed');
    // Refresh preview after build
    preview.refresh();
  } catch (err) {
    console.error('Build failed:', err);
    alert(`Build failed: ${err}`);
  } finally {
    buildBtn.disabled = false;
    buildBtn.textContent = 'Build';
  }
}

// Setup resizable split pane
function setupResizer() {
  let isResizing = false;
  let startX = 0;
  let startEditorWidth = 0;

  resizer.addEventListener('mousedown', (e) => {
    isResizing = true;
    startX = e.clientX;
    startEditorWidth = editorPane.offsetWidth;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;

    const dx = e.clientX - startX;
    const newWidth = startEditorWidth + dx;
    const containerWidth = editorPane.parentElement!.offsetWidth;
    const minWidth = 300;
    const maxWidth = containerWidth - 200 - resizer.offsetWidth;

    if (newWidth >= minWidth && newWidth <= maxWidth) {
      editorPane.style.flex = `0 0 ${newWidth}px`;
      previewPane.style.flex = '1 1 auto';
    }
  });

  document.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
  });
}

// File tree rendering
function renderFileTree(entries: FileEntry[]): string {
  return entries.map(entry => {
    if (entry.is_dir) {
      return `
        <div class="tree-folder">
          <div class="tree-item" data-folder="${entry.path}">
            <span class="icon">▶</span>
            <span class="name">${entry.name}</span>
          </div>
          <div class="tree-children">
            ${entry.children ? renderFileTree(entry.children) : ''}
          </div>
        </div>
      `;
    } else {
      return `
        <div class="tree-item tree-file" data-slug="${entry.path}">
          <span class="icon">◇</span>
          <span class="name">${entry.name.replace(/\.md$/, '')}</span>
        </div>
      `;
    }
  }).join('');
}

async function loadFileTree() {
  try {
    const response = await api.listFiles();
    if (response.files.length === 0) {
      fileTree.innerHTML = '<div class="loading">No files in vault</div>';
    } else {
      fileTree.innerHTML = renderFileTree(response.files);
      setupTreeListeners();
    }
  } catch (err) {
    console.error('Failed to load files:', err);
    fileTree.innerHTML = '<div class="loading">Failed to load files</div>';
  }
}

function setupTreeListeners() {
  // Folder click - toggle open/close
  fileTree.querySelectorAll('[data-folder]').forEach(el => {
    el.addEventListener('click', (e) => {
      e.stopPropagation();
      const folder = el.closest('.tree-folder');
      if (folder) {
        folder.classList.toggle('open');
        const icon = el.querySelector('.icon');
        if (icon) {
          icon.textContent = folder.classList.contains('open') ? '▼' : '▶';
        }
      }
    });
  });

  // File click - open note
  fileTree.querySelectorAll('[data-slug]').forEach(el => {
    el.addEventListener('click', () => {
      const slug = el.getAttribute('data-slug');
      if (slug) {
        // Update active state
        fileTree.querySelectorAll('.tree-item').forEach(item => item.classList.remove('active'));
        el.classList.add('active');

        // Update slug input and open
        slugInput.value = slug;
        openNote(slug);
      }
    });
  });
}

function updateActiveFile(slug: string) {
  fileTree.querySelectorAll('.tree-item').forEach(item => {
    item.classList.remove('active');
    if (item.getAttribute('data-slug') === slug) {
      item.classList.add('active');
    }
  });
}

// Setup sidebar resizer
function setupSidebarResizer() {
  let isResizing = false;
  let startX = 0;
  let startWidth = 0;

  sidebarResizer.addEventListener('mousedown', (e) => {
    isResizing = true;
    startX = e.clientX;
    startWidth = sidebar.offsetWidth;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;

    const dx = e.clientX - startX;
    const newWidth = startWidth + dx;

    if (newWidth >= 150 && newWidth <= 400) {
      sidebar.style.width = `${newWidth}px`;
    }
  });

  document.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
  });
}

// New file handler
newFileBtn.addEventListener('click', () => {
  const name = prompt('New file name (without .md):');
  if (name && name.trim()) {
    const slug = name.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
    slugInput.value = slug;
    openNote(slug);
    // Reload file tree after a short delay to pick up the new file
    setTimeout(loadFileTree, 1000);
  }
});

// Event listeners
openBtn.addEventListener('click', () => openNote(slugInput.value));

slugInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    openNote(slugInput.value);
  }
});

checkpointBtn.addEventListener('click', doCheckpoint);
buildBtn.addEventListener("click", doBuild);

flushBtn.addEventListener("click", async () => {
  flushBtn.disabled = true;
  flushBtn.textContent = "Saving...";
  try {
    const res = await api.flush();
    console.log("Flushed:", res);
    alert(res.message || "Flushed dirty docs");
  } catch (err) {
    console.error("Flush failed:", err);
    alert(`Flush failed: ${err}`);
  } finally {
    flushBtn.disabled = false;
    flushBtn.textContent = "Save";
  }
});

refreshBtn.addEventListener('click', () => {
  preview.refresh();
});

previewUrlInput.addEventListener('change', () => {
  preview.setBaseUrl(previewUrlInput.value);
  savePreferences();
});

// Initialize
loadPreferences();
setupResizer();
setupSidebarResizer();
loadFileTree();

// Auto-open last slug if present
if (slugInput.value) {
  openNote(slugInput.value);
  updateActiveFile(slugInput.value);
}
