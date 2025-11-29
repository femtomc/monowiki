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
 * Filter marks that should be preserved when splitting a block at a given position.
 * Returns marks that end before or at the split position.
 */
export function filterMarksBeforeSplit(marks: Mark[], splitPos: number): Mark[] {
  return marks.filter((m) => m.end <= splitPos);
}

/**
 * Filter and shift marks that should be preserved after a split.
 * Returns marks that start at or after the split position, with positions shifted.
 */
export function filterMarksAfterSplit(marks: Mark[], splitPos: number): Mark[] {
  return marks
    .filter((m) => m.start >= splitPos)
    .map((m) => ({
      ...m,
      start: m.start - splitPos,
      end: m.end - splitPos,
    }));
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
  const preservedStartMarks = filterMarksBeforeSplit(startMarks, relStart);
  const preservedEndMarks = filterMarksAfterSplit(endMarks, relEndInEndBlock);

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
