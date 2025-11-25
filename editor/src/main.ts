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
import { CollabAPI } from './api';
import { Preview } from './preview';

// DOM elements
const slugInput = document.getElementById('slug-input') as HTMLInputElement;
const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
const connectionStatus = document.getElementById('connection-status') as HTMLSpanElement;
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

// State
let currentEditor: EditorInstance | null = null;
let currentSlug: string | null = null;

// API client - in dev mode, Vite proxies /api and /ws to localhost:8787
const api = new CollabAPI('');

// Preview manager
const preview = new Preview({
  iframe: previewFrame,
  baseUrl: previewUrlInput.value || 'http://localhost:3000',
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
    alert(`Flush failed: `);
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

// Auto-open last slug if present
if (slugInput.value) {
  openNote(slugInput.value);
}
