/**
 * monowiki-editor main entry point.
 *
 * Wires together:
 * - CodeMirror 6 editor with Loro-based sync
 * - Split pane with resizer
 * - Preview iframe (points to dev server)
 * - Toolbar actions (open, checkpoint, build, refresh)
 */

import { createEditor, EditorInstance, ConnectionStatus } from './editor';
import { CollabAPI, FileEntry, Comment } from './api';
import { Preview } from './preview';
import { AgentPanel } from './agent-panel';
import { LoroDoc, LoroMap, LoroList } from 'loro-crdt';
import type { ViewUpdate } from '@codemirror/view';

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
const commentsList = document.getElementById('comments-list') as HTMLDivElement;
const commentsRefresh = document.getElementById('comments-refresh') as HTMLButtonElement;

// Sidebar elements
const fileTree = document.getElementById('file-tree') as HTMLDivElement;
const newFileBtn = document.getElementById('new-file-btn') as HTMLButtonElement;
const sidebar = document.getElementById('sidebar') as HTMLElement;
const sidebarResizer = document.getElementById('sidebar-resizer') as HTMLDivElement;

// State
type BlockRange = {
  id: string;
  kind: string;
  blockStart: number;
  blockEnd: number;
  textStart: number;
  textEnd: number;
  text: string;
};

let currentEditor: EditorInstance | null = null;
let currentSlug: string | null = null;
let renderTimeout: number | null = null;
let loroDoc: LoroDoc | null = null;
let loroUnsub: (() => void) | null = null;
let collabSocket: WebSocket | null = null;
let blockRanges: BlockRange[] = [];
let suppressEditorUpdate = false;
let commentsCache: Comment[] = [];

const RENDER_DEBOUNCE_MS = 100; // Wait 100ms after typing stops before rendering

function findBlockForOffset(offset: number): BlockRange | null {
  if (!blockRanges.length) return null;
  const hit = blockRanges.find((b) => offset >= b.textStart && offset <= b.textEnd);
  if (hit) return hit;
  // Fallback to closest preceding block to keep selections usable near boundaries
  let candidate: BlockRange | null = null;
  for (const b of blockRanges) {
    if (offset >= b.blockStart && offset <= b.blockEnd) {
      return b;
    }
    if (b.blockStart <= offset) {
      candidate = b;
    } else {
      break;
    }
  }
  return candidate;
}

type BlockData = {
  id: string;
  kind: string;
  attrs?: Record<string, unknown>;
  text?: string;
};

function blockText(block: BlockData, texts: LoroMap<any>): string {
  const fromMap = texts.get(block.id);
  if (typeof fromMap === 'string') return fromMap;
  return block.text ?? '';
}

function toMarkdownFromDoc(doc: LoroDoc): { markdown: string; ranges: BlockRange[] } {
  const blocks: LoroList<any> = doc.getList('blocks');
  const texts: LoroMap<any> = doc.getMap('texts');
  let output = '';
  const ranges: BlockRange[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const raw = blocks.get(i);
    if (typeof raw !== 'string') continue;

    let block: BlockData | null = null;
    try {
      block = JSON.parse(raw) as BlockData;
    } catch {
      continue;
    }
    if (!block) continue;

    const text = blockText(block, texts);

    const blockStart = output.length;
    let contentStart = blockStart;
    let blockEnd;

    switch (block.kind) {
      case 'heading': {
        const level = typeof block.attrs?.level === 'number' ? block.attrs.level : 1;
        const prefix = '#'.repeat(Math.max(1, Math.min(6, level)));
        output += `${prefix} `;
        contentStart = output.length;
        output += `${text}\n\n`;
        blockEnd = output.length;
        break;
      }
      case 'code_block': {
        const lang = typeof block.attrs?.language === 'string' ? block.attrs.language : '';
        output += `\`\`\`${lang}\n`;
        contentStart = output.length;
        output += `${text}\n\`\`\`\n\n`;
        blockEnd = output.length;
        break;
      }
      case 'blockquote': {
        const quoted = text.split('\n').map((line) => `> ${line}`).join('\n');
        contentStart = output.length + 2; // "> "
        output += `${quoted}\n\n`;
        blockEnd = output.length;
        break;
      }
      case 'thematic_break': {
        contentStart = output.length;
        output += '---\n\n';
        blockEnd = output.length;
        break;
      }
      case 'math_block': {
        output += '$$\n';
        contentStart = output.length;
        output += `${text}\n$$\n\n`;
        blockEnd = output.length;
        break;
      }
      case 'list_item':
      case 'bullet_list':
      case 'ordered_list': {
        output += `- `;
        contentStart = output.length;
        output += `${text}\n`;
        blockEnd = output.length;
        break;
      }
      default: {
        contentStart = output.length;
        output += `${text}\n\n`;
        blockEnd = output.length;
      }
    }

    const contentEnd = contentStart + text.length;
    ranges.push({
      id: block.id,
      kind: block.kind,
      blockStart,
      blockEnd,
      textStart: contentStart,
      textEnd: contentEnd,
      text,
    });
  }

  return { markdown: output.trimEnd(), ranges };
}

function updateBlockText(blockId: string, newText: string) {
  if (!loroDoc) return;
  const blocks = loroDoc.getList('blocks');
  for (let i = 0; i < blocks.length; i++) {
    const raw = blocks.get(i);
    if (typeof raw !== 'string') continue;
    try {
      const block = JSON.parse(raw) as BlockData;
      if (block.id === blockId) {
        block.text = newText;
        blocks.delete(i, 1);
        blocks.insert(i, JSON.stringify(block));
        return;
      }
    } catch {
      continue;
    }
  }
}

function deleteBlockById(blockId: string) {
  if (!loroDoc) return;
  const blocks = loroDoc.getList('blocks');
  for (let i = 0; i < blocks.length; i++) {
    const raw = blocks.get(i);
    if (typeof raw !== 'string') continue;
    try {
      const block = JSON.parse(raw) as BlockData;
      if (block.id === blockId) {
        blocks.delete(i, 1);
        break;
      }
    } catch {
      continue;
    }
  }
  // Drop text entry
  const texts = loroDoc.getMap('texts');
  texts.delete(blockId as any);
  // Drop comments anchored to this block
  const comments = loroDoc.getMap('comments');
  for (const key of comments.keys()) {
    const raw = comments.get(key);
    if (typeof raw !== 'string') continue;
    try {
      const c = JSON.parse(raw);
      if (c.block_id === blockId) {
        comments.delete(key);
      }
    } catch {
      continue;
    }
  }
}

function adjustCommentsForChange(blockId: string, start: number, end: number, insert: string) {
  if (!loroDoc) return;
  const comments = loroDoc.getMap('comments');
  const keys = comments.keys();
  const deleteLen = Math.max(0, end - start);
  const insertLen = insert.length;
  const deleteEnd = start + deleteLen;

  for (const key of keys) {
    const raw = comments.get(key);
    if (typeof raw !== 'string') continue;
    let comment: any;
    try {
      comment = JSON.parse(raw);
    } catch {
      continue;
    }
    if (comment.block_id !== blockId) continue;

    let changed = false;

    if (insertLen > 0) {
      if (start <= comment.start) {
        comment.start += insertLen;
        comment.end += insertLen;
        changed = true;
      } else if (start < comment.end) {
        comment.end += insertLen;
        changed = true;
      }
    }

    if (deleteLen > 0) {
      if (comment.end <= start) {
        // no-op
      } else if (comment.start >= deleteEnd) {
        comment.start -= deleteLen;
        comment.end -= deleteLen;
        changed = true;
      } else if (comment.start >= start && comment.end <= deleteEnd) {
        comments.delete(key);
        continue;
      } else if (comment.start < start && comment.end > deleteEnd) {
        comment.end -= deleteLen;
        changed = true;
      }
    }

    if (changed) {
      comments.set(key, JSON.stringify(comment));
    }
  }
}

function refreshEditorFromDoc(forceText = false) {
  if (!loroDoc || !currentEditor) return;
  const { markdown, ranges } = toMarkdownFromDoc(loroDoc);
  blockRanges = ranges;

  const currentText = currentEditor.view.state.doc.toString();
  if (forceText || currentText !== markdown) {
    suppressEditorUpdate = true;
    currentEditor.view.dispatch({
      changes: { from: 0, to: currentEditor.view.state.doc.length, insert: markdown },
    });
    suppressEditorUpdate = false;
  }
}

function applyBlockEdit(blockId: string, start: number, end: number, insertText: string) {
  if (!loroDoc) return;
  const texts = loroDoc.getMap('texts');
  const current = texts.get(blockId);
  const prev = typeof current === 'string' ? current : '';
  const next = prev.slice(0, start) + insertText + prev.slice(end);

  texts.set(blockId, next);
  updateBlockText(blockId, next);
  adjustCommentsForChange(blockId, start, end, insertText);
  loroDoc.commit();
  refreshEditorFromDoc();
}

function applyEditorChange(update: ViewUpdate) {
  if (suppressEditorUpdate || !loroDoc) return;

  let applied = false;
  update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    const block = findBlockForOffset(fromA);
    const insertText = inserted.toString();
    const endBlock = findBlockForOffset(Math.max(toA - 1, fromA));

    if (!block || !endBlock) {
      refreshEditorFromDoc(true);
      return;
    }

    // Cross-block edit: merge into start block and delete consumed blocks
    if (block.id !== endBlock.id || toA > block.textEnd) {
      const relStart = Math.max(0, fromA - block.textStart);
      const relEndEndBlock = Math.max(0, toA - endBlock.textStart);

      if (!loroDoc) return;
      const texts = loroDoc.getMap('texts');
      const startText = typeof texts.get(block.id) === 'string' ? (texts.get(block.id) as string) : block.text;
      const endText = typeof texts.get(endBlock.id) === 'string' ? (texts.get(endBlock.id) as string) : endBlock.text;

      const prefix = startText.slice(0, relStart);
      const suffix = endText.slice(relEndEndBlock);
      const merged = prefix + insertText + suffix;

      applyBlockEdit(block.id, 0, startText.length, merged);

      // Remove any blocks fully covered by the selection (after the start block)
      const startIdx = blockRanges.findIndex((b) => b.id === block.id);
      const endIdx = blockRanges.findIndex((b) => b.id === endBlock.id);
      if (startIdx >= 0 && endIdx >= startIdx) {
        for (let idx = endIdx; idx > startIdx; idx--) {
          deleteBlockById(blockRanges[idx].id);
        }
      }

      applied = true;
      return;
    }

    const relStart = Math.max(0, fromA - block.textStart);
    const relEnd = Math.max(0, toA - block.textStart);
    applyBlockEdit(block.id, relStart, relEnd, insertText);
    applied = true;
  });

  if (applied) {
    scheduleRender();
  }
}

function formatTimestamp(ts: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return d.toLocaleString();
}

function renderComments() {
  if (!commentsList) return;
  if (!commentsCache.length) {
    commentsList.innerHTML = '<div class="comment-empty">No comments yet</div>';
    return;
  }

  commentsList.innerHTML = commentsCache.map((c) => {
    const status = c.resolved ? '<span class="comment-tag">resolved</span>' : '';
    return `
      <div class="comment-card" data-comment-id="${c.id}">
        <div class="comment-meta">
          <span>${c.author || 'unknown'}</span>
          <span>${formatTimestamp(c.created_at)}</span>
        </div>
        <div class="comment-body">${c.content}</div>
        <div class="comment-tags">
          <span class="comment-tag">block ${c.block_id}</span>
          <span class="comment-tag">range ${c.start}-${c.end}</span>
          ${status}
        </div>
        <div class="comment-actions">
          ${c.resolved ? '' : '<button class="resolve" data-comment-id="' + c.id + '">Resolve</button>'}
        </div>
      </div>
    `;
  }).join('');

  commentsList.querySelectorAll('button.resolve').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const id = btn.getAttribute('data-comment-id');
      if (!id || !currentSlug) return;
      btn.setAttribute('disabled', 'true');
      try {
        await api.resolveComment(currentSlug, id);
        await loadComments();
      } catch (err) {
        console.error('Resolve failed:', err);
        btn.removeAttribute('disabled');
      }
    });
  });
}

async function loadComments() {
  if (!currentSlug) {
    commentsCache = [];
    renderComments();
    return;
  }
  try {
    const res = await api.getComments(currentSlug);
    commentsCache = res.comments;
    renderComments();
  } catch (err) {
    console.error('Failed to load comments:', err);
    commentsList.innerHTML = '<div class="comment-empty">Failed to load comments</div>';
  }
}

function disconnectCollab() {
  if (collabSocket) {
    collabSocket.close();
    collabSocket = null;
  }
  if (loroUnsub) {
    loroUnsub();
    loroUnsub = null;
  }
  loroDoc = null;
  blockRanges = [];
  updateStatusDisplay('disconnected');
}

function connectCollab(slug: string) {
  disconnectCollab();

  loroDoc = new LoroDoc();
  loroUnsub = loroDoc.subscribeLocalUpdates((bytes: Uint8Array) => {
    if (collabSocket && collabSocket.readyState === WebSocket.OPEN) {
      collabSocket.send(bytes);
    }
  });

  const wsBaseUrl = api.wsBaseUrl();
  const token = tokenInput.value ? `?token=${encodeURIComponent(tokenInput.value)}` : '';
  const url = `${wsBaseUrl}/${slug}${token}`;

  collabSocket = new WebSocket(url);
  collabSocket.binaryType = 'arraybuffer';

  updateStatusDisplay('connecting');

  collabSocket.onopen = () => updateStatusDisplay('connected');
  collabSocket.onclose = () => updateStatusDisplay('disconnected');
  collabSocket.onerror = () => updateStatusDisplay('disconnected');
  collabSocket.onmessage = (event) => {
    if (typeof event.data === 'string') {
      return;
    }

    const bytes = new Uint8Array(event.data as ArrayBuffer);
    if (!bytes.length) return;

    try {
      loroDoc?.import(bytes);
      refreshEditorFromDoc(true);
      // Comments may have changed if other peers/agent added them
      loadComments();
    } catch (err) {
      console.error('Failed to import Loro update', err);
    }
  };
}

// API client - in dev mode, Vite proxies /api and /ws to localhost:8787
const api = new CollabAPI('');

// Preview manager - defaults to /preview (served by collab daemon)
const preview = new Preview({
  iframe: previewFrame,
  baseUrl: previewUrlInput.value || '/preview',
});

// Agent panel - self-registers event listeners and keyboard shortcuts
new AgentPanel({
  container: document.body,
  api,
  getSelection: () => {
    if (!currentEditor) return null;
    const view = currentEditor.view;
    const { from, to } = view.state.selection.main;
    if (from === to) return null;

    const text = view.state.sliceDoc(from, to);
    const block = findBlockForOffset(from);
    if (!block) return null;

    return {
      text,
      block_id: block.id,
      start: Math.max(0, from - block.textStart),
      end: Math.max(0, to - block.textStart),
    };
  },
  getCurrentSlug: () => currentSlug,
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

  // Clean up existing editor + collab session
  if (currentEditor) {
    currentEditor.destroy();
    currentEditor = null;
  }
  disconnectCollab();

  currentSlug = slug;
  savePreferences();

  try {
    // Save token to API + local storage
    if (tokenInput.value) {
      api.setToken(tokenInput.value);
      savePreferences();
    }

    currentEditor = createEditor({
      container: editorContainer,
      onContentChange: applyEditorChange,
    });

    connectCollab(slug);

    // Navigate preview
    preview.navigate(slug);
    loadComments();

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

commentsRefresh.addEventListener('click', () => {
  loadComments();
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
