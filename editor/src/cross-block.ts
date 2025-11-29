/**
 * Cross-block editing operations for structure-preserving edits.
 *
 * This module provides functions for:
 * - Parsing markdown into block structures
 * - Transforming cross-block selections while preserving document structure
 */

// =============================================================================
// Types
// =============================================================================

export type ParsedBlock = {
  kind: string;
  attrs: Record<string, unknown>;
  text: string;
};

export type BlockData = {
  id: string;
  kind: string;
  attrs?: Record<string, unknown>;
  text?: string;
};

export type Mark = {
  mark_type: string;
  start: number;
  end: number;
  start_anchor: 'before' | 'after';
  end_anchor: 'before' | 'after';
  attrs: Record<string, unknown>;
};

export type Comment = {
  id: string;
  block_id: string;
  start: number;
  end: number;
  content: string;
  author: string;
  created_at: string;
  resolved: boolean;
  parent_id?: string;
  /** Set when comment was migrated from a deleted block */
  migrated_from?: string;
};

// =============================================================================
// Markdown Parser
// =============================================================================

/**
 * Parse markdown text into block structures.
 * Mirrors the server-side parse_markdown_to_blocks in crdt.rs.
 */
export function parseMarkdownToBlocks(markdown: string): ParsedBlock[] {
  const blocks: ParsedBlock[] = [];
  const lines = markdown.split('\n');
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Heading
    if (line.startsWith('#')) {
      const level = line.match(/^#+/)?.[0].length ?? 1;
      const text = line.slice(level).trim();
      blocks.push({ kind: 'heading', attrs: { level }, text });
      i++;
      continue;
    }

    // Code block
    if (line.startsWith('```') || line.startsWith('~~~')) {
      const fence = line.startsWith('```') ? '```' : '~~~';
      const lang = line.slice(fence.length).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith(fence)) {
        codeLines.push(lines[i]);
        i++;
      }
      i++; // skip closing fence
      const attrs: Record<string, unknown> = {};
      if (lang) attrs.language = lang;
      blocks.push({ kind: 'code_block', attrs, text: codeLines.join('\n') });
      continue;
    }

    // Math block
    if (line.startsWith('$$')) {
      const mathLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith('$$')) {
        mathLines.push(lines[i]);
        i++;
      }
      i++; // skip closing $$
      blocks.push({ kind: 'math_block', attrs: {}, text: mathLines.join('\n') });
      continue;
    }

    // Thematic break
    if (/^(---|\*\*\*|___)$/.test(line.trim())) {
      blocks.push({ kind: 'thematic_break', attrs: {}, text: '' });
      i++;
      continue;
    }

    // Blockquote
    if (line.startsWith('>')) {
      const quoteLines: string[] = [line.slice(1).trim()];
      i++;
      while (i < lines.length && lines[i].startsWith('>')) {
        quoteLines.push(lines[i].slice(1).trim());
        i++;
      }
      blocks.push({ kind: 'blockquote', attrs: {}, text: quoteLines.join('\n') });
      continue;
    }

    // List item (unordered)
    if (/^[-*+] /.test(line)) {
      blocks.push({ kind: 'list_item', attrs: {}, text: line.slice(2) });
      i++;
      continue;
    }

    // List item (ordered)
    const orderedMatch = line.match(/^\d+\. (.*)$/);
    if (orderedMatch) {
      blocks.push({ kind: 'list_item', attrs: {}, text: orderedMatch[1] });
      i++;
      continue;
    }

    // Empty line - skip
    if (line.trim() === '') {
      i++;
      continue;
    }

    // Paragraph - collect consecutive non-empty, non-special lines
    const paraLines: string[] = [line];
    i++;
    while (i < lines.length) {
      const nextLine = lines[i];
      if (
        nextLine.trim() === '' ||
        nextLine.startsWith('#') ||
        nextLine.startsWith('```') ||
        nextLine.startsWith('~~~') ||
        nextLine.startsWith('$$') ||
        nextLine.startsWith('>') ||
        /^[-*+] /.test(nextLine) ||
        /^\d+\. /.test(nextLine) ||
        /^(---|\*\*\*|___)$/.test(nextLine.trim())
      ) {
        break;
      }
      paraLines.push(nextLine);
      i++;
    }
    blocks.push({ kind: 'paragraph', attrs: {}, text: paraLines.join('\n') });
  }

  return blocks;
}

// =============================================================================
// Mark Operations
// =============================================================================

/**
 * Clip marks for the prefix portion of a block when splitting at splitPos.
 * - Marks entirely before splitPos: kept as-is
 * - Marks spanning splitPos: clipped to end at splitPos
 * - Marks entirely after splitPos: dropped (in deleted portion)
 */
export function clipMarksForPrefix(marks: Mark[], splitPos: number): Mark[] {
  const result: Mark[] = [];
  for (const m of marks) {
    if (m.end <= splitPos) {
      // Entirely before split - keep as-is
      result.push({ ...m });
    } else if (m.start < splitPos) {
      // Spans the split - clip to end at splitPos
      result.push({
        ...m,
        end: splitPos,
      });
    }
    // else: entirely after split - drop
  }
  return result;
}

/**
 * Clip marks for the suffix portion of a block when splitting at splitPos.
 * - Marks entirely before splitPos: dropped (in deleted portion)
 * - Marks spanning splitPos: clipped to start at 0 (was splitPos), end shifted
 * - Marks entirely after splitPos: shifted by -splitPos
 */
export function clipMarksForSuffix(marks: Mark[], splitPos: number): Mark[] {
  const result: Mark[] = [];
  for (const m of marks) {
    if (m.start >= splitPos) {
      // Entirely after split - shift
      result.push({
        ...m,
        start: m.start - splitPos,
        end: m.end - splitPos,
      });
    } else if (m.end > splitPos) {
      // Spans the split - clip to start at 0
      result.push({
        ...m,
        start: 0,
        end: m.end - splitPos,
      });
    }
    // else: entirely before split - drop
  }
  return result;
}

/**
 * Legacy filter function - kept for backwards compatibility.
 * @deprecated Use clipMarksForPrefix instead
 */
export function filterMarksBeforeSplit(marks: Mark[], splitPos: number): Mark[] {
  return clipMarksForPrefix(marks, splitPos);
}

/**
 * Legacy filter function - kept for backwards compatibility.
 * @deprecated Use clipMarksForSuffix instead
 */
export function filterMarksAfterSplit(marks: Mark[], splitPos: number): Mark[] {
  return clipMarksForSuffix(marks, splitPos);
}

/**
 * Merge two sets of marks, shifting the second set by an offset.
 */
export function mergeMarks(
  firstMarks: Mark[],
  secondMarks: Mark[],
  offset: number,
): Mark[] {
  return [
    ...firstMarks,
    ...secondMarks.map((m) => ({
      ...m,
      start: m.start + offset,
      end: m.end + offset,
    })),
  ];
}

// =============================================================================
// Comment Operations
// =============================================================================

/**
 * Adjust comments on the start block after a cross-block edit.
 * - Comments entirely before relStart: kept as-is
 * - Comments spanning relStart: clipped to end at relStart
 * - Comments entirely in deleted portion (>= relStart): dropped
 */
export function adjustCommentsForStartBlock(
  comments: Comment[],
  blockId: string,
  relStart: number,
): Comment[] {
  const result: Comment[] = [];
  for (const c of comments) {
    if (c.block_id !== blockId) {
      result.push(c);
      continue;
    }
    if (c.end <= relStart) {
      // Entirely before selection - keep as-is
      result.push({ ...c });
    } else if (c.start < relStart) {
      // Spans the selection start - clip
      result.push({
        ...c,
        end: relStart,
      });
    }
    // else: entirely in deleted portion - drop
  }
  return result;
}

/**
 * Adjust comments on the end block after a cross-block edit.
 * The suffix text (from relEnd onwards) will be placed at newSuffixStart.
 * - Comments entirely before relEnd: dropped (in deleted portion)
 * - Comments spanning relEnd: clipped to start at newSuffixStart
 * - Comments entirely after relEnd: shifted to new position
 */
export function adjustCommentsForEndBlock(
  comments: Comment[],
  blockId: string,
  relEnd: number,
  newSuffixStart: number,
): Comment[] {
  const result: Comment[] = [];
  for (const c of comments) {
    if (c.block_id !== blockId) {
      result.push(c);
      continue;
    }
    if (c.start >= relEnd) {
      // Entirely after selection end - shift to new position
      const shift = newSuffixStart - relEnd;
      result.push({
        ...c,
        start: c.start + shift,
        end: c.end + shift,
      });
    } else if (c.end > relEnd) {
      // Spans the selection end - clip and shift
      const shift = newSuffixStart - relEnd;
      result.push({
        ...c,
        start: newSuffixStart,
        end: c.end + shift,
      });
    }
    // else: entirely in deleted portion - drop
  }
  return result;
}

/**
 * Migrate comments from deleted interior blocks to a surviving block.
 * Comments are tagged with migrated_from to indicate they were moved.
 */
export function migrateCommentsFromDeletedBlocks(
  comments: Comment[],
  deletedBlockIds: Set<string>,
  targetBlockId: string,
  targetOffset: number,
): Comment[] {
  const result: Comment[] = [];
  for (const c of comments) {
    if (deletedBlockIds.has(c.block_id)) {
      // Migrate to target block
      result.push({
        ...c,
        block_id: targetBlockId,
        start: targetOffset,
        end: targetOffset, // Collapsed to a point since original text is gone
        migrated_from: c.block_id,
      });
    } else {
      result.push(c);
    }
  }
  return result;
}

// =============================================================================
// Cross-Block Transform Planning
// =============================================================================

export interface CrossBlockPlan {
  /** Prefix text to keep in the start block */
  prefix: string;
  /** Suffix text to keep (goes to last new block or end block) */
  suffix: string;
  /** Parsed blocks from inserted text */
  newBlocks: ParsedBlock[];
  /** IDs of blocks to delete (interior blocks) */
  deleteBlockIds: string[];
  /** Marks to preserve from start block */
  preservedStartMarks: Mark[];
  /** Marks to preserve from end block (already shifted to 0-based) */
  preservedEndMarks: Mark[];
}

/**
 * Plan a cross-block transformation without executing it.
 * This is useful for testing the transformation logic.
 */
export function planCrossBlockTransform(
  startBlockText: string,
  endBlockText: string,
  relStart: number,
  relEndInEndBlock: number,
  insertText: string,
  startMarks: Mark[] = [],
  endMarks: Mark[] = [],
  interiorBlockIds: string[] = [],
): CrossBlockPlan {
  const prefix = startBlockText.slice(0, relStart);
  const suffix = endBlockText.slice(relEndInEndBlock);
  const newBlocks = parseMarkdownToBlocks(insertText);
  // Use clipping functions that properly handle marks spanning the selection boundary
  const preservedStartMarks = clipMarksForPrefix(startMarks, relStart);
  const preservedEndMarks = clipMarksForSuffix(endMarks, relEndInEndBlock);

  return {
    prefix,
    suffix,
    newBlocks,
    deleteBlockIds: interiorBlockIds,
    preservedStartMarks,
    preservedEndMarks,
  };
}

/**
 * Determine the final block structure after applying a cross-block transform.
 * Returns a description of what the resulting blocks should look like.
 */
export interface ResultBlock {
  originalId?: string; // ID if this is a modified existing block
  kind: string;
  text: string;
  marks: Mark[];
}

export function computeResultBlocks(
  plan: CrossBlockPlan,
  startBlock: BlockData,
): ResultBlock[] {
  const results: ResultBlock[] = [];

  if (plan.newBlocks.length === 0) {
    // No new blocks - merge prefix + suffix into start block
    results.push({
      originalId: startBlock.id,
      kind: startBlock.kind,
      text: plan.prefix + plan.suffix,
      marks: mergeMarks(plan.preservedStartMarks, plan.preservedEndMarks, plan.prefix.length),
    });
  } else if (plan.newBlocks.length === 1 && plan.newBlocks[0].kind === startBlock.kind) {
    // Single block of same kind - merge everything
    const insertedLen = plan.newBlocks[0].text.length;
    results.push({
      originalId: startBlock.id,
      kind: startBlock.kind,
      text: plan.prefix + plan.newBlocks[0].text + plan.suffix,
      marks: mergeMarks(
        plan.preservedStartMarks,
        plan.preservedEndMarks,
        plan.prefix.length + insertedLen,
      ),
    });
  } else {
    // Multiple blocks or different kind
    let firstBlockIdx = 0;

    if (plan.newBlocks[0].kind === startBlock.kind) {
      // First new block merges with start block
      results.push({
        originalId: startBlock.id,
        kind: startBlock.kind,
        text: plan.prefix + plan.newBlocks[0].text,
        marks: plan.preservedStartMarks,
      });
      firstBlockIdx = 1;
    } else {
      // Start block keeps only prefix
      results.push({
        originalId: startBlock.id,
        kind: startBlock.kind,
        text: plan.prefix,
        marks: plan.preservedStartMarks,
      });
    }

    // Insert remaining new blocks
    for (let i = firstBlockIdx; i < plan.newBlocks.length; i++) {
      const pb = plan.newBlocks[i];
      const isLast = i === plan.newBlocks.length - 1;
      const blockText = isLast ? pb.text + plan.suffix : pb.text;
      const blockMarks = isLast
        ? plan.preservedEndMarks.map((m) => ({
            ...m,
            start: m.start + pb.text.length,
            end: m.end + pb.text.length,
          }))
        : [];

      results.push({
        kind: pb.kind,
        text: blockText,
        marks: blockMarks,
      });
    }

    // If no parsed blocks handled the suffix, create a paragraph
    if (plan.newBlocks.length === firstBlockIdx && plan.suffix) {
      results.push({
        kind: 'paragraph',
        text: plan.suffix,
        marks: plan.preservedEndMarks,
      });
    }
  }

  return results;
}
