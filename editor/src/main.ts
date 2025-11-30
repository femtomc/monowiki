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
import { CollabAPI, FileEntry, Comment, Selection, SearchHit } from './api';
import { Preview } from './preview';
import { AgentPanel } from './agent-panel';
import { LoroDoc, LoroMap, LoroList } from 'loro-crdt';
import type { ViewUpdate } from '@codemirror/view';
import {
  parseMarkdownToBlocks,
  clipMarksForPrefix,
  clipMarksForSuffix,
  type Mark,
} from './cross-block';

// DOM elements
const slugInput = document.getElementById('slug-input') as HTMLInputElement;
const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
const connectionStatus = document.getElementById('connection-status') as HTMLSpanElement;
const flushBtn = document.getElementById('flush-btn') as HTMLButtonElement;
const checkpointBtn = document.getElementById('checkpoint-btn') as HTMLButtonElement;
const buildBtn = document.getElementById('build-btn') as HTMLButtonElement;
const previewUrlInput = document.getElementById('preview-url') as HTMLInputElement;
const refreshBtn = document.getElementById('refresh-btn') as HTMLButtonElement;
const statusBanner = document.getElementById('status-banner') as HTMLDivElement;
const editorContainer = document.getElementById('editor') as HTMLDivElement;
const previewFrame = document.getElementById('preview-frame') as HTMLIFrameElement;
const resizer = document.getElementById('resizer') as HTMLDivElement;
const editorPane = document.querySelector('.editor-pane') as HTMLDivElement;
const previewPane = document.querySelector('.preview-pane') as HTMLDivElement;
const tokenInput = document.getElementById('token-input') as HTMLInputElement;
const commentsList = document.getElementById('comments-list') as HTMLDivElement;
const commentsRefresh = document.getElementById('comments-refresh') as HTMLButtonElement;
const addCommentBtn = document.getElementById('add-comment-btn') as HTMLButtonElement;
const backlinksList = document.getElementById('backlinks-list') as HTMLDivElement;
const backlinksRefresh = document.getElementById('backlinks-refresh') as HTMLButtonElement;
const commentsPane = document.getElementById('comments-pane') as HTMLDivElement;
const relatedList = document.getElementById('related-list') as HTMLDivElement;
const searchInput = document.getElementById('search-input') as HTMLInputElement;
const searchBtn = document.getElementById('search-btn') as HTMLButtonElement;
const searchResultsList = document.getElementById('search-results') as HTMLDivElement;
const eventsList = document.getElementById('events-list') as HTMLDivElement;
const togglePreviewBtn = document.getElementById('toggle-preview-btn') as HTMLButtonElement;
const toggleContextBtn = document.getElementById('toggle-context-btn') as HTMLButtonElement;
const publishingBtn = document.getElementById('publishing-btn') as HTMLButtonElement;
const publishingPanel = document.getElementById('publishing-panel') as HTMLDivElement;
const contextMenu = document.getElementById('context-menu') as HTMLDivElement;

// Sidebar elements
const fileTree = document.getElementById('file-tree') as HTMLDivElement;
const newFileBtn = document.getElementById('new-file-btn') as HTMLButtonElement;
const sidebar = document.getElementById('sidebar') as HTMLElement;
const sidebarResizer = document.getElementById('sidebar-resizer') as HTMLDivElement;
const newFileForm = document.getElementById('new-file-form') as HTMLDivElement;
const newFileInput = document.getElementById('new-file-input') as HTMLInputElement;
const newFileConfirm = document.getElementById('create-file-confirm') as HTMLButtonElement;
const newFileCancel = document.getElementById('create-file-cancel') as HTMLButtonElement;

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
let agentPanel: AgentPanel | null = null;
let statusTimeout: number | null = null;
let backlinksCache: string[] = [];
let outgoingCache: string[] = [];
let searchResults: SearchHit[] = [];
let recentEvents: { event: string; slug: string; path?: string; id?: string }[] = [];
let previewVisible = false;
let contextVisible = false;
let publishingOpen = false;
let contextMenuOpen = false;
let composerAnchor: { block_id: string; start: number; end: number; from: number; to: number } | null = null;
let editingCommentId: string | null = null;
let floatingComposer: HTMLDivElement | null = null;
let floatingComposerTextarea: HTMLTextAreaElement | null = null;

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

/**
 * Generate a unique block ID for client-side operations.
 * Uses crypto.randomUUID when available, falls back to timestamp + random.
 * Prefixed with 'cb_' to distinguish from server-generated IDs.
 */
function nextClientBlockId(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return `cb_${crypto.randomUUID()}`;
  }
  // Fallback: timestamp + random
  return `cb_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`;
}

// =============================================================================
// Block Operations for Cross-Block Edits
// =============================================================================

interface CrossBlockTransformResult {
  success: boolean;
  error?: string;
}

/**
 * Structure-preserving cross-block edit transform.
 *
 * For a selection spanning blocks [startBlock...endBlock]:
 * 1. Split startBlock at selection start → keep prefix in startBlock
 * 2. Split endBlock at selection end → keep suffix in endBlock
 * 3. Migrate comments from interior blocks to start block
 * 4. Delete fully covered interior blocks
 * 5. Parse inserted text into new blocks
 * 6. Insert new blocks after startBlock (or merge into startBlock if same kind)
 * 7. Adjust marks and comments to maintain correct anchoring
 */
function applyCrossBlockTransform(
  startBlockId: string,
  endBlockId: string,
  relStart: number,
  relEndInEndBlock: number,
  insertText: string,
): CrossBlockTransformResult {
  if (!loroDoc) return { success: false, error: 'No Loro doc' };

  try {
    const blocks = loroDoc.getList('blocks');
    const texts = loroDoc.getMap('texts');
    const marks = loroDoc.getMap('marks');
    const comments = loroDoc.getMap('comments');

    // Find block indices
    let startIdx = -1;
    let endIdx = -1;
    const blockList: BlockData[] = [];

    for (let i = 0; i < blocks.length; i++) {
      const raw = blocks.get(i);
      if (typeof raw !== 'string') continue;
      try {
        const block = JSON.parse(raw) as BlockData;
        blockList.push(block);
        if (block.id === startBlockId) startIdx = blockList.length - 1;
        if (block.id === endBlockId) endIdx = blockList.length - 1;
      } catch {
        continue;
      }
    }

    if (startIdx === -1 || endIdx === -1) {
      return { success: false, error: 'Block not found' };
    }

    const startBlock = blockList[startIdx];
    const endBlock = blockList[endIdx];

    // Get current text content
    const startText = typeof texts.get(startBlockId) === 'string'
      ? (texts.get(startBlockId) as string)
      : (startBlock.text ?? '');
    const endText = typeof texts.get(endBlockId) === 'string'
      ? (texts.get(endBlockId) as string)
      : (endBlock.text ?? '');

    // Calculate prefix (before selection in start block) and suffix (after selection in end block)
    const prefix = startText.slice(0, relStart);
    const suffix = endText.slice(relEndInEndBlock);

    // Parse inserted text into blocks
    const parsedBlocks = parseMarkdownToBlocks(insertText);

    // Handle marks from startBlock - clip marks that span the selection boundary
    const startMarksRaw = marks.get(startBlockId);
    let startMarks: Mark[] = [];
    if (typeof startMarksRaw === 'string') {
      try {
        startMarks = JSON.parse(startMarksRaw);
      } catch { /* ignore */ }
    }
    const preservedStartMarks = clipMarksForPrefix(startMarks, relStart);

    // Handle marks from endBlock - clip marks that span the selection boundary
    const endMarksRaw = marks.get(endBlockId);
    let endMarks: Mark[] = [];
    if (typeof endMarksRaw === 'string') {
      try {
        endMarks = JSON.parse(endMarksRaw);
      } catch { /* ignore */ }
    }
    const preservedEndMarks = clipMarksForSuffix(endMarks, relEndInEndBlock);

    // Collect interior block IDs for migration
    const deletedBlockIds = new Set<string>();
    for (let i = startIdx + 1; i <= endIdx; i++) {
      deletedBlockIds.add(blockList[i].id);
    }

    // Migrate comments from interior blocks to start block at the prefix end
    // Comments are collapsed to nearby points to avoid stacking exactly
    let migrationOffset = 0;
    for (const key of comments.keys()) {
      const raw = comments.get(key);
      if (typeof raw !== 'string') continue;
      try {
        const c = JSON.parse(raw);
        if (deletedBlockIds.has(c.block_id)) {
          const originalBlockId = c.block_id;
          // Migrate to start block at prefix position, mark as migrated
          const targetPos = relStart + migrationOffset;
          c.block_id = startBlockId;
          c.start = targetPos;
          c.end = targetPos;
          c.migrated_from = originalBlockId;
          comments.set(key, JSON.stringify(c));
          migrationOffset += 1;
        }
      } catch { /* ignore */ }
    }

    // Adjust comments on start block: clip those spanning relStart
    for (const key of comments.keys()) {
      const raw = comments.get(key);
      if (typeof raw !== 'string') continue;
      try {
        const c = JSON.parse(raw);
        if (c.block_id !== startBlockId) continue;

        if (c.end <= relStart) {
          // Entirely before selection - keep as-is
        } else if (c.start < relStart && c.end > relStart) {
          // Spans the selection start - clip to end at relStart
          c.end = relStart;
          comments.set(key, JSON.stringify(c));
        } else if (c.start >= relStart && !c.migrated_from) {
          // Entirely in deleted portion of start block - delete
          comments.delete(key);
        }
      } catch { /* ignore */ }
    }

    // Delete interior blocks (in reverse order to maintain indices)
    for (let i = endIdx; i > startIdx; i--) {
      const blockId = blockList[i].id;
      blocks.delete(i, 1);
      texts.delete(blockId as any);
      marks.delete(blockId as any);
    }

    // Calculate where the suffix will end up for comment adjustment
    let suffixBlockId = startBlockId;
    let suffixStartOffset = prefix.length;

    // Now handle the merged content
    if (parsedBlocks.length === 0) {
      // No new blocks - merge prefix + suffix into startBlock
      const mergedText = prefix + suffix;
      texts.set(startBlockId, mergedText);
      updateBlockText(startBlockId, mergedText);

      // Merge marks: start marks + shifted end marks
      const mergedMarks = [
        ...preservedStartMarks,
        ...preservedEndMarks.map((m: Mark) => ({
          ...m,
          start: m.start + prefix.length,
          end: m.end + prefix.length,
        })),
      ];
      marks.set(startBlockId, JSON.stringify(mergedMarks));

      suffixStartOffset = prefix.length;

    } else if (parsedBlocks.length === 1 && parsedBlocks[0].kind === startBlock.kind) {
      // Single block of same kind - merge everything into startBlock
      const mergedText = prefix + parsedBlocks[0].text + suffix;
      texts.set(startBlockId, mergedText);
      updateBlockText(startBlockId, mergedText);

      // Merge marks
      const insertedLen = parsedBlocks[0].text.length;
      const mergedMarks = [
        ...preservedStartMarks,
        ...preservedEndMarks.map((m: Mark) => ({
          ...m,
          start: m.start + prefix.length + insertedLen,
          end: m.end + prefix.length + insertedLen,
        })),
      ];
      marks.set(startBlockId, JSON.stringify(mergedMarks));

      suffixStartOffset = prefix.length + insertedLen;

    } else {
      // Multiple blocks or different kind - insert new blocks
      let firstBlockIdx = 0;

      if (parsedBlocks[0].kind === startBlock.kind) {
        const newStartText = prefix + parsedBlocks[0].text;
        texts.set(startBlockId, newStartText);
        updateBlockText(startBlockId, newStartText);
        marks.set(startBlockId, JSON.stringify(preservedStartMarks));
        firstBlockIdx = 1;
      } else {
        texts.set(startBlockId, prefix);
        updateBlockText(startBlockId, prefix);
        marks.set(startBlockId, JSON.stringify(preservedStartMarks));
      }

      // Insert middle blocks
      let insertPos = startIdx + 1;
      for (let i = firstBlockIdx; i < parsedBlocks.length; i++) {
        const pb = parsedBlocks[i];
        const isLast = i === parsedBlocks.length - 1;

        const blockText = isLast ? pb.text + suffix : pb.text;
        const newId = nextClientBlockId();

        const newBlockData: BlockData = {
          id: newId,
          kind: pb.kind,
          attrs: pb.attrs,
          text: blockText,
        };

        blocks.insert(insertPos, JSON.stringify(newBlockData));
        texts.set(newId, blockText);

        if (isLast) {
          const shiftedEndMarks = preservedEndMarks.map((m: Mark) => ({
            ...m,
            start: m.start + pb.text.length,
            end: m.end + pb.text.length,
          }));
          marks.set(newId, JSON.stringify(shiftedEndMarks));
          suffixBlockId = newId;
          suffixStartOffset = pb.text.length;
        }

        insertPos++;
      }

      // If no parsed blocks to append suffix to, create a paragraph for it
      if (parsedBlocks.length === firstBlockIdx && suffix) {
        const newId = nextClientBlockId();
        const newBlockData: BlockData = {
          id: newId,
          kind: 'paragraph',
          attrs: {},
          text: suffix,
        };
        blocks.insert(insertPos, JSON.stringify(newBlockData));
        texts.set(newId, suffix);
        marks.set(newId, JSON.stringify(preservedEndMarks));
        suffixBlockId = newId;
        suffixStartOffset = 0;
      }
    }

    // Adjust comments from end block that are now in the suffix
    // These need to be re-anchored to wherever the suffix ended up
    for (const key of comments.keys()) {
      const raw = comments.get(key);
      if (typeof raw !== 'string') continue;
      try {
        const c = JSON.parse(raw);
        if (c.block_id !== endBlockId) continue;

        if (c.start >= relEndInEndBlock) {
          // Entirely after selection end - shift to new position in suffix block
          const shift = suffixStartOffset - relEndInEndBlock;
          c.block_id = suffixBlockId;
          c.start = c.start + shift;
          c.end = c.end + shift;
          comments.set(key, JSON.stringify(c));
        } else if (c.end > relEndInEndBlock) {
          // Spans the selection end - clip and shift
          c.block_id = suffixBlockId;
          c.start = suffixStartOffset;
          c.end = c.end - relEndInEndBlock + suffixStartOffset;
          comments.set(key, JSON.stringify(c));
        }
        // else: entirely in deleted portion - already handled or will be orphaned
      } catch { /* ignore */ }
    }

    loroDoc.commit();
    refreshEditorFromDoc();
    return { success: true };

  } catch (err) {
    console.error('Cross-block transform failed:', err);
    return { success: false, error: String(err) };
  }
}

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

    // Cross-block edit: use structure-preserving transform
    if (block.id !== endBlock.id || toA > block.textEnd) {
      const relStart = Math.max(0, fromA - block.textStart);
      const relEndEndBlock = Math.max(0, toA - endBlock.textStart);

      // Try structure-preserving transform first
      const result = applyCrossBlockTransform(
        block.id,
        endBlock.id,
        relStart,
        relEndEndBlock,
        insertText,
      );

      if (result.success) {
        applied = true;
        return;
      }

      // If the structural transform fails, refresh from doc and abort to avoid divergence.
      console.error('Cross-block transform failed:', result.error);
      showStatus(`Cross-block edit failed: ${result.error ?? 'unknown error'}. Document refreshed.`, true);
      refreshEditorFromDoc(true);
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

function showStatus(msg: string, isWarning = false) {
  if (!statusBanner) return;
  statusBanner.textContent = msg;
  statusBanner.hidden = false;
  statusBanner.style.background = isWarning ? '#fff4e5' : '#e8f5e9';
  statusBanner.style.color = isWarning ? '#8a6d00' : '#2d7a3d';
  if (statusTimeout !== null) {
    clearTimeout(statusTimeout);
  }
  statusTimeout = window.setTimeout(() => {
    statusBanner.hidden = true;
  }, 5000);
}

function clearStatus() {
  if (!statusBanner) return;
  statusBanner.hidden = true;
  if (statusTimeout !== null) {
    clearTimeout(statusTimeout);
    statusTimeout = null;
  }
}

function formatTimestamp(ts: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return d.toLocaleString();
}

function getCurrentSelection(): Selection | null {
  if (!currentEditor) return null;
  const view = currentEditor.view;
  const { from, to } = view.state.selection.main;
  if (from === to) return null;

  const block = findBlockForOffset(from);
  if (!block) return null;

  return {
    text: view.state.sliceDoc(from, to),
    block_id: block.id,
    start: Math.max(0, from - block.textStart),
    end: Math.max(0, to - block.textStart),
  };
}

function updateSelectionDependentUI() {
  if (!currentSlug) {
    addCommentBtn.disabled = true;
    return;
  }
  addCommentBtn.disabled = !getCurrentSelection();
}

function getBlockText(blockId: string): string {
  if (loroDoc) {
    const texts = loroDoc.getMap('texts');
    const value = texts.get(blockId);
    if (typeof value === 'string') return value;
  }
  const range = blockRanges.find((b) => b.id === blockId);
  return range?.text ?? '';
}

function getCommentRange(comment: Comment): { from: number; to: number } | null {
  const block = blockRanges.find((b) => b.id === comment.block_id);
  if (!block) return null;
  const from = block.textStart + comment.start;
  const to = block.textStart + comment.end;
  return { from, to };
}

function getAnchorSnippet(comment: Comment, maxLen = 140): string {
  const text = getBlockText(comment.block_id);
  const snippet = text.slice(comment.start, comment.end).trim();
  if (!snippet) return '';
  if (snippet.length <= maxLen) return snippet;
  return `${snippet.slice(0, maxLen)}…`;
}

function renderComments() {
  if (!commentsList) return;
  if (!commentsCache.length) {
    commentsList.innerHTML = '<div class="comment-empty">No comments yet</div>';
    return;
  }

  commentsList.innerHTML = commentsCache.map((c) => {
    const status = c.resolved ? '<span class="comment-tag">resolved</span>' : '';
    const migrated = c.migrated_from
      ? `<span class="comment-tag migrated">migrated from ${c.migrated_from}</span>`
      : '';
    const anchorSnippet = getAnchorSnippet(c);
    const anchor = anchorSnippet ? `<div class="comment-anchor">“${anchorSnippet}”</div>` : '';
    return `
      <div class="comment-card" data-comment-id="${c.id}">
        <div class="comment-meta">
          <span>${c.author || 'unknown'}</span>
          <span>${formatTimestamp(c.created_at)}</span>
        </div>
        ${anchor}
        <div class="comment-body">${c.content}</div>
        <div class="comment-tags">
          <span class="comment-tag">block ${c.block_id}</span>
          <span class="comment-tag">range ${c.start}-${c.end}</span>
          ${status}
          ${migrated}
        </div>
        <div class="comment-actions">
          <button class="jump" data-comment-id="${c.id}">Jump</button>
          ${c.resolved ? '' : '<button class="resolve" data-comment-id="' + c.id + '">Resolve</button>'}
          ${c.resolved ? '' : '<button class="edit" data-comment-id="' + c.id + '">Edit</button>'}
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

  commentsList.querySelectorAll('button.jump').forEach((btn) => {
    btn.addEventListener('click', () => {
      const id = btn.getAttribute('data-comment-id');
      const comment = commentsCache.find((c) => c.id === id);
      if (!comment || !currentEditor) return;
      const range = getCommentRange(comment);
      if (!range) return;
      currentEditor.view.dispatch({
        selection: { anchor: range.from, head: range.to },
        scrollIntoView: true,
      });
    });
  });

  commentsList.querySelectorAll('button.edit').forEach((btn) => {
    btn.addEventListener('click', () => {
      const id = btn.getAttribute('data-comment-id');
      const comment = commentsCache.find((c) => c.id === id && !c.resolved);
      if (!comment) return;
      openComposerForComment(comment);
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

function renderContextList(target: HTMLElement, items: string[], emptyText: string) {
  if (!items.length) {
    target.innerHTML = `<div class="context-empty">${emptyText}</div>`;
    return;
  }
  target.innerHTML = items
    .map((slug) => {
      const label = slug.replace(/\.md$/, '');
      return `<div class="context-item" data-slug="${slug}">
        <div class="title">${label}</div>
        <div class="meta">open</div>
      </div>`;
    })
    .join('');

  target.querySelectorAll<HTMLElement>('[data-slug]').forEach((el) => {
    el.addEventListener('click', () => {
      const slug = el.getAttribute('data-slug');
      if (slug) {
        slugInput.value = slug;
        openNote(slug);
      }
    });
  });
}

function renderSearchResults() {
  if (!searchResultsList) return;
  if (!searchResults.length) {
    searchResultsList.innerHTML = '<div class="context-empty">No matches</div>';
    return;
  }
  searchResultsList.innerHTML = searchResults
    .map((hit) => {
      const label = hit.section_title ? `${hit.title} — ${hit.section_title}` : hit.title;
      return `<div class="context-item" data-slug="${hit.slug}">
        <div class="title">${label}</div>
        <div class="meta">${hit.doc_type}${hit.tags.length ? ' • ' + hit.tags.join(', ') : ''}</div>
        <div class="meta">${hit.snippet}</div>
      </div>`;
    })
    .join('');

  searchResultsList.querySelectorAll<HTMLElement>('[data-slug]').forEach((el) => {
    el.addEventListener('click', () => {
      const slug = el.getAttribute('data-slug');
      if (slug) {
        slugInput.value = slug;
        openNote(slug);
      }
    });
  });
}

function renderEvents() {
  if (!eventsList) return;
  if (!recentEvents.length) {
    eventsList.innerHTML = '<div class="context-empty">No events</div>';
    return;
  }
  eventsList.innerHTML = recentEvents
    .slice(-10)
    .reverse()
    .map((ev) => {
      const title = ev.event.replace(/^note\./, 'note ').replace(/^comment\./, 'comment ');
      const meta = ev.path || ev.id || '';
      return `<div class="context-item">
        <div class="title">${title}</div>
        <div class="meta">${meta}</div>
      </div>`;
    })
    .join('');
}

async function loadGraph() {
  if (!currentSlug) {
    backlinksCache = [];
    outgoingCache = [];
    renderContextList(backlinksList, [], 'No backlinks');
    renderContextList(relatedList, [], 'No related notes');
    recentEvents = [];
    renderEvents();
    return;
  }
  try {
    const res = await api.getGraph(currentSlug);
    backlinksCache = res.backlinks;
    outgoingCache = res.outgoing;
    renderContextList(backlinksList, backlinksCache, 'No backlinks yet');
    const related = Array.from(new Set([...backlinksCache, ...outgoingCache])).filter(
      (s) => s !== currentSlug,
    );
    renderContextList(relatedList, related, 'No related notes');
    await loadEvents();
  } catch (err) {
    console.error('Failed to load graph:', err);
    renderContextList(backlinksList, [], 'Failed to load');
    renderContextList(relatedList, [], 'Failed to load');
    eventsList.innerHTML = '<div class="context-empty">Failed to load</div>';
  }
}

async function loadEvents() {
  if (!currentSlug) {
    recentEvents = [];
    renderEvents();
    return;
  }
  try {
    const res = await api.getEvents(currentSlug);
    recentEvents = res.events.slice(-20).map((e) => {
      try {
        return JSON.parse(e);
      } catch {
        return { event: 'unknown', raw: e };
      }
    });
    renderEvents();
  } catch (err) {
    console.error('Failed to load events:', err);
    eventsList.innerHTML = '<div class="context-empty">Failed to load</div>';
  }
}

async function runSearch() {
  const query = searchInput.value.trim();
  if (!query) {
    searchResults = [];
    renderSearchResults();
    return;
  }
  try {
    const res = await api.search(query, 12);
    searchResults = res.results;
    renderSearchResults();
  } catch (err) {
    console.error('Search failed:', err);
    searchResultsList.innerHTML = '<div class="context-empty">Search failed</div>';
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

function setNoteActive(active: boolean) {
  togglePreviewBtn.disabled = !active;
  toggleContextBtn.disabled = !active;
  flushBtn.disabled = !active;
  checkpointBtn.disabled = !active;
  buildBtn.disabled = !active;
  refreshBtn.disabled = !active;
  previewUrlInput.disabled = !active;
  addCommentBtn.disabled = !active;
  commentsRefresh.disabled = !active;
  backlinksRefresh.disabled = !active;
  searchInput.disabled = !active;
  searchBtn.disabled = !active;

  if (!active) {
    previewVisible = false;
    contextVisible = false;
    publishingOpen = false;
    publishingPanel.classList.add('hidden');
    updatePreviewVisibility();
    updateContextVisibility();
  }
}

function updatePreviewVisibility() {
  if (previewVisible && currentSlug) {
    resizer.classList.remove('collapsed');
    previewPane.classList.remove('collapsed');
    togglePreviewBtn.textContent = 'Hide preview';
  } else {
    resizer.classList.add('collapsed');
    previewPane.classList.add('collapsed');
    togglePreviewBtn.textContent = 'Show preview';
    editorPane.style.flex = '1 1 auto';
  }
}

function updateContextVisibility() {
  if (contextVisible && currentSlug) {
    commentsPane.classList.remove('collapsed');
    toggleContextBtn.textContent = 'Hide context';
  } else {
    commentsPane.classList.add('collapsed');
    toggleContextBtn.textContent = 'Show context';
  }
}

function ensureAgentPanel() {
  if (agentPanel) return;
  agentPanel = new AgentPanel({
    container: document.body,
    api,
    getSelection: () => getCurrentSelection(),
    getCurrentSlug: () => currentSlug,
  });
}

function closePublishingPanel() {
  publishingOpen = false;
  publishingPanel.classList.add('hidden');
}

function togglePublishingPanel() {
  if (publishingBtn.disabled) return;
  publishingOpen = !publishingOpen;
  publishingPanel.classList.toggle('hidden', !publishingOpen);
}

type MenuItem = { label: string; action: () => void; disabled?: boolean };

function hideContextMenu() {
  contextMenuOpen = false;
  contextMenu.classList.add('hidden');
}

function showContextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  contextMenu.innerHTML = '';
  items.forEach((item) => {
    const btn = document.createElement('button');
    btn.textContent = item.label;
    btn.disabled = !!item.disabled;
    if (!item.disabled) {
      btn.addEventListener('click', () => {
        hideContextMenu();
        item.action();
      });
    }
    contextMenu.appendChild(btn);
  });
  contextMenu.style.left = `${e.clientX}px`;
  contextMenu.style.top = `${e.clientY}px`;
  contextMenu.classList.remove('hidden');
  contextMenuOpen = true;
}

function showNewFileForm() {
  newFileForm.classList.remove('hidden');
  newFileInput.value = '';
  newFileInput.focus();
}

function hideNewFileForm() {
  newFileForm.classList.add('hidden');
}

function normalizeSlug(input: string): string {
  return input.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
}

function submitNewFile() {
  const name = normalizeSlug(newFileInput.value);
  if (!name) {
    showStatus('Enter a file name', true);
    return;
  }
  slugInput.value = name;
  openNote(name);
  hideNewFileForm();
  setTimeout(loadFileTree, 1000);
}

function ensureFloatingComposer() {
  if (floatingComposer) return;
  floatingComposer = document.createElement('div');
  floatingComposer.className = 'floating-comment hidden';
  floatingComposer.innerHTML = `
    <div class="anchor-preview" id="floating-anchor"></div>
    <textarea id="floating-textarea" placeholder="Add a comment"></textarea>
    <div class="actions">
      <button id="floating-cancel" class="secondary">Cancel</button>
      <button id="floating-submit">Add</button>
    </div>
  `;
  document.body.appendChild(floatingComposer);
  floatingComposerTextarea = floatingComposer.querySelector('#floating-textarea') as HTMLTextAreaElement;

  const cancelBtn = floatingComposer.querySelector('#floating-cancel') as HTMLButtonElement;
  const submitBtn = floatingComposer.querySelector('#floating-submit') as HTMLButtonElement;
  cancelBtn.addEventListener('click', hideFloatingComposer);
  submitBtn.addEventListener('click', submitFloatingComment);
  floatingComposerTextarea?.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      hideFloatingComposer();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      submitFloatingComment();
    }
  });
}

function hideFloatingComposer() {
  if (!floatingComposer) return;
  floatingComposer.classList.add('hidden');
  composerAnchor = null;
  editingCommentId = null;
}

function openComposerForComment(comment: Comment) {
  const range = getCommentRange(comment);
  if (!range || !currentEditor) {
    showStatus('Cannot locate comment anchor', true);
    return;
  }
  const selection: Selection = {
    text: getAnchorSnippet(comment) || comment.content,
    block_id: comment.block_id,
    start: comment.start,
    end: comment.end,
  };
  currentEditor.view.dispatch({
    selection: { anchor: range.from, head: range.to },
    scrollIntoView: true,
  });
  showFloatingComposer(selection, comment);
}

function showFloatingComposer(sel: Selection | null, commentToEdit?: Comment) {
  if (!sel || !currentSlug) {
    showStatus('Select text to comment', true);
    return;
  }
  ensureFloatingComposer();
  if (!floatingComposer || !floatingComposerTextarea) return;

  const range = getCommentRange({
    id: '',
    block_id: sel.block_id,
    start: sel.start,
    end: sel.end,
    content: '',
    author: '',
    created_at: '',
    resolved: false,
  } as Comment);

  if (!range || !currentEditor) return;

  composerAnchor = { block_id: sel.block_id, start: sel.start, end: sel.end, from: range.from, to: range.to };
  editingCommentId = commentToEdit?.id ?? null;
  const anchorEl = floatingComposer.querySelector('#floating-anchor') as HTMLDivElement;
  anchorEl.textContent = sel.text;
  floatingComposerTextarea.value = commentToEdit ? commentToEdit.content : sel.text;

  const startCoords = currentEditor.view.coordsAtPos(range.from);
  const endCoords = currentEditor.view.coordsAtPos(range.to);
  const top = Math.min(startCoords?.top ?? 0, endCoords?.top ?? 0) - 10 + window.scrollY;
  const left = (startCoords?.left ?? 0) + window.scrollX;

  floatingComposer.classList.remove('hidden');
  const safeLeft = Math.min(Math.max(12, left), window.scrollX + window.innerWidth - floatingComposer.offsetWidth - 12);

  floatingComposer.style.top = `${Math.max(12, top)}px`;
  floatingComposer.style.left = `${safeLeft}px`;

  floatingComposerTextarea.focus();
  floatingComposerTextarea.select();
}

async function submitFloatingComment() {
  if (!composerAnchor || !floatingComposerTextarea || !currentSlug) return;
  const content = floatingComposerTextarea.value.trim();
  if (!content) {
    showStatus('Comment cannot be empty', true);
    return;
  }
  try {
    await api.addComment(currentSlug, {
      block_id: composerAnchor.block_id,
      start: composerAnchor.start,
      end: composerAnchor.end,
      content,
    });
    if (editingCommentId) {
      await api.resolveComment(currentSlug, editingCommentId);
    }
    await loadComments();
    showStatus(editingCommentId ? 'Comment updated' : 'Comment added');
  } catch (err) {
    console.error('Failed to add comment:', err);
    showStatus('Failed to add comment', true);
  } finally {
    hideFloatingComposer();
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
  clearStatus();

  // Clean up existing editor + collab session
  if (currentEditor) {
    currentEditor.destroy();
    currentEditor = null;
  }
  disconnectCollab();

  currentSlug = slug;
  savePreferences();
  setNoteActive(false);

  try {
    // Save token to API + local storage
    if (tokenInput.value) {
      api.setToken(tokenInput.value);
      savePreferences();
    }

    currentEditor = createEditor({
      container: editorContainer,
      onContentChange: applyEditorChange,
      onSelectionChange: updateSelectionDependentUI,
    });
    // Expose editor view for automated tests
    if (typeof window !== 'undefined') {
      (window as any).__cm_view = currentEditor.view;
    }

    connectCollab(slug);

    // Navigate preview
    preview.navigate(slug);
    loadComments();
    loadGraph();
    renderEvents();
    ensureAgentPanel();
    setNoteActive(true);
    updatePreviewVisibility();
    updateContextVisibility();
    updateSelectionDependentUI();
    closePublishingPanel();

  } catch (err) {
    console.error('Failed to open note:', err);
    updateStatusDisplay('disconnected');
    alert(`Failed to open note: ${err}`);
    setNoteActive(false);
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
  showNewFileForm();
});

// Event listeners
openBtn.addEventListener('click', () => openNote(slugInput.value));

slugInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    openNote(slugInput.value);
  }
});

togglePreviewBtn.addEventListener('click', () => {
  if (!currentSlug) return;
  previewVisible = !previewVisible;
  updatePreviewVisibility();
  if (previewVisible) {
    preview.navigate(currentSlug);
  }
});

toggleContextBtn.addEventListener('click', () => {
  if (!currentSlug) return;
  contextVisible = !contextVisible;
  updateContextVisibility();
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

addCommentBtn.addEventListener('click', () => {
  showFloatingComposer(getCurrentSelection());
});

backlinksRefresh.addEventListener('click', () => {
  loadGraph();
});

searchBtn.addEventListener('click', () => {
  runSearch();
});

searchInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    runSearch();
  }
});

publishingBtn.addEventListener('click', () => {
  togglePublishingPanel();
});

document.addEventListener('click', (e) => {
  const target = e.target as HTMLElement;
  if (publishingOpen && !publishingPanel.contains(target) && target !== publishingBtn && !publishingBtn.contains(target)) {
    closePublishingPanel();
  }
  if (contextMenuOpen && !contextMenu.contains(target)) {
    hideContextMenu();
  }
  if (floatingComposer && !floatingComposer.classList.contains('hidden') && !floatingComposer.contains(target)) {
    hideFloatingComposer();
  }
});

newFileConfirm.addEventListener('click', submitNewFile);
newFileCancel.addEventListener('click', hideNewFileForm);
newFileInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    submitNewFile();
  } else if (e.key === 'Escape') {
    hideNewFileForm();
  }
});

// Context menu bindings
sidebar.addEventListener('contextmenu', (e) => {
  showContextMenu(e, [
    { label: 'New file', action: showNewFileForm },
    { label: 'Refresh files', action: loadFileTree },
  ]);
});

editorContainer.addEventListener('contextmenu', (e) => {
  const selection = getCurrentSelection();
  showContextMenu(e, [
    { label: 'Add comment', action: () => showFloatingComposer(selection), disabled: !selection || !currentSlug },
  ]);
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    hideContextMenu();
    hideFloatingComposer();
  }
});

// Initialize
setNoteActive(false);
loadPreferences();
setupResizer();
setupSidebarResizer();
loadFileTree();

// Auto-open last slug if present
if (slugInput.value) {
  openNote(slugInput.value);
  updateActiveFile(slugInput.value);
}
