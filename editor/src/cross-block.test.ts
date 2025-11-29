/**
 * Tests for cross-block editing operations.
 *
 * Exercises:
 * - Multi-block delete (para→para)
 * - Paste creating new paragraphs into a selection spanning blocks
 * - Edits inside lists/code blocks across block boundaries
 * - Mark preservation through splits/merges
 */

import { describe, it, expect } from 'vitest';
import {
  parseMarkdownToBlocks,
  filterMarksBeforeSplit,
  filterMarksAfterSplit,
  clipMarksForPrefix,
  clipMarksForSuffix,
  mergeMarks,
  adjustCommentsForStartBlock,
  adjustCommentsForEndBlock,
  migrateCommentsFromDeletedBlocks,
  planCrossBlockTransform,
  computeResultBlocks,
  type Mark,
  type BlockData,
  type Comment,
} from './cross-block';

// =============================================================================
// Markdown Parser Tests
// =============================================================================

describe('parseMarkdownToBlocks', () => {
  it('parses a heading', () => {
    const blocks = parseMarkdownToBlocks('# Hello World');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('heading');
    expect(blocks[0].text).toBe('Hello World');
    expect(blocks[0].attrs.level).toBe(1);
  });

  it('parses multiple heading levels', () => {
    const blocks = parseMarkdownToBlocks('## Level 2\n### Level 3');
    expect(blocks).toHaveLength(2);
    expect(blocks[0].attrs.level).toBe(2);
    expect(blocks[1].attrs.level).toBe(3);
  });

  it('parses a paragraph', () => {
    const blocks = parseMarkdownToBlocks('This is a paragraph.');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('paragraph');
    expect(blocks[0].text).toBe('This is a paragraph.');
  });

  it('parses multi-line paragraphs', () => {
    const blocks = parseMarkdownToBlocks('Line one\nLine two\nLine three');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('paragraph');
    expect(blocks[0].text).toBe('Line one\nLine two\nLine three');
  });

  it('parses code blocks with language', () => {
    const blocks = parseMarkdownToBlocks('```typescript\nconst x = 1;\n```');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('code_block');
    expect(blocks[0].text).toBe('const x = 1;');
    expect(blocks[0].attrs.language).toBe('typescript');
  });

  it('parses code blocks without language', () => {
    const blocks = parseMarkdownToBlocks('```\nplain code\n```');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('code_block');
    expect(blocks[0].text).toBe('plain code');
    expect(blocks[0].attrs.language).toBeUndefined();
  });

  it('parses blockquotes', () => {
    const blocks = parseMarkdownToBlocks('> Quote line 1\n> Quote line 2');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('blockquote');
    expect(blocks[0].text).toBe('Quote line 1\nQuote line 2');
  });

  it('parses list items', () => {
    const blocks = parseMarkdownToBlocks('- Item 1\n- Item 2\n* Item 3');
    expect(blocks).toHaveLength(3);
    expect(blocks[0].kind).toBe('list_item');
    expect(blocks[0].text).toBe('Item 1');
    expect(blocks[1].text).toBe('Item 2');
    expect(blocks[2].text).toBe('Item 3');
  });

  it('parses ordered list items', () => {
    const blocks = parseMarkdownToBlocks('1. First\n2. Second');
    expect(blocks).toHaveLength(2);
    expect(blocks[0].kind).toBe('list_item');
    expect(blocks[0].text).toBe('First');
  });

  it('parses thematic breaks', () => {
    const blocks = parseMarkdownToBlocks('---');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('thematic_break');
    expect(blocks[0].text).toBe('');
  });

  it('parses math blocks', () => {
    const blocks = parseMarkdownToBlocks('$$\nx^2 + y^2 = z^2\n$$');
    expect(blocks).toHaveLength(1);
    expect(blocks[0].kind).toBe('math_block');
    expect(blocks[0].text).toBe('x^2 + y^2 = z^2');
  });

  it('parses mixed content', () => {
    const md = `# Heading

This is a paragraph.

\`\`\`js
code();
\`\`\`

- List item`;
    const blocks = parseMarkdownToBlocks(md);
    expect(blocks).toHaveLength(4);
    expect(blocks[0].kind).toBe('heading');
    expect(blocks[1].kind).toBe('paragraph');
    expect(blocks[2].kind).toBe('code_block');
    expect(blocks[3].kind).toBe('list_item');
  });

  it('handles empty input', () => {
    const blocks = parseMarkdownToBlocks('');
    expect(blocks).toHaveLength(0);
  });

  it('handles only whitespace', () => {
    const blocks = parseMarkdownToBlocks('   \n\n   ');
    expect(blocks).toHaveLength(0);
  });
});

// =============================================================================
// Mark Operation Tests
// =============================================================================

describe('filterMarksBeforeSplit (legacy - now clips)', () => {
  const marks: Mark[] = [
    { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'italic', start: 3, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'code', start: 8, end: 12, start_anchor: 'before', end_anchor: 'after', attrs: {} },
  ];

  it('keeps marks ending before split and clips marks spanning split', () => {
    const filtered = filterMarksBeforeSplit(marks, 5);
    // Now clips, so bold [0,5] is kept, italic [3,10] is clipped to [3,5]
    expect(filtered).toHaveLength(2);
    expect(filtered[0].mark_type).toBe('bold');
    expect(filtered[1].mark_type).toBe('italic');
    expect(filtered[1].end).toBe(5); // clipped
  });

  it('clips marks that span split position', () => {
    const filtered = filterMarksBeforeSplit(marks, 3);
    // Bold [0,5] spans 3, so it gets clipped to [0,3]
    expect(filtered).toHaveLength(1);
    expect(filtered[0].mark_type).toBe('bold');
    expect(filtered[0].end).toBe(3);
  });

  it('includes marks ending exactly at split plus clipped ones', () => {
    const filtered = filterMarksBeforeSplit(marks, 10);
    // bold [0,5], italic [3,10] kept, code [8,12] clipped to [8,10]
    expect(filtered).toHaveLength(3);
  });
});

describe('filterMarksAfterSplit (legacy - now clips)', () => {
  const marks: Mark[] = [
    { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'italic', start: 5, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'code', start: 8, end: 12, start_anchor: 'before', end_anchor: 'after', attrs: {} },
  ];

  it('keeps and shifts marks starting at or after split', () => {
    const filtered = filterMarksAfterSplit(marks, 5);
    expect(filtered).toHaveLength(2);
    expect(filtered[0].mark_type).toBe('italic');
    expect(filtered[0].start).toBe(0); // shifted from 5
    expect(filtered[0].end).toBe(5); // shifted from 10
  });

  it('clips marks spanning split and shifts marks after', () => {
    const filtered = filterMarksAfterSplit(marks, 8);
    // italic [5,10] spans 8, clipped to [0,2]; code [8,12] shifted to [0,4]
    expect(filtered).toHaveLength(2);
    const italic = filtered.find((m) => m.mark_type === 'italic');
    expect(italic).toBeDefined();
    expect(italic?.start).toBe(0);
    expect(italic?.end).toBe(2); // 10 - 8 = 2

    const code = filtered.find((m) => m.mark_type === 'code');
    expect(code).toBeDefined();
    expect(code?.start).toBe(0);
    expect(code?.end).toBe(4);
  });
});

describe('mergeMarks', () => {
  it('combines two mark sets with offset', () => {
    const first: Mark[] = [
      { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const second: Mark[] = [
      { mark_type: 'italic', start: 0, end: 3, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const merged = mergeMarks(first, second, 10);
    expect(merged).toHaveLength(2);
    expect(merged[0].start).toBe(0);
    expect(merged[1].start).toBe(10);
    expect(merged[1].end).toBe(13);
  });
});

// =============================================================================
// Mark Clipping Tests (new behavior - clips instead of drops)
// =============================================================================

describe('clipMarksForPrefix', () => {
  const marks: Mark[] = [
    { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'italic', start: 3, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'code', start: 8, end: 12, start_anchor: 'before', end_anchor: 'after', attrs: {} },
  ];

  it('keeps marks entirely before split as-is', () => {
    const clipped = clipMarksForPrefix(marks, 6);
    const bold = clipped.find((m) => m.mark_type === 'bold');
    expect(bold).toBeDefined();
    expect(bold?.start).toBe(0);
    expect(bold?.end).toBe(5);
  });

  it('clips marks spanning the split position', () => {
    const clipped = clipMarksForPrefix(marks, 6);
    const italic = clipped.find((m) => m.mark_type === 'italic');
    expect(italic).toBeDefined();
    expect(italic?.start).toBe(3);
    expect(italic?.end).toBe(6); // clipped from 10 to 6
  });

  it('drops marks entirely after split', () => {
    const clipped = clipMarksForPrefix(marks, 6);
    const code = clipped.find((m) => m.mark_type === 'code');
    expect(code).toBeUndefined();
  });

  it('returns empty for split at 0', () => {
    const clipped = clipMarksForPrefix(marks, 0);
    expect(clipped).toHaveLength(0);
  });
});

describe('clipMarksForSuffix', () => {
  const marks: Mark[] = [
    { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'italic', start: 3, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'code', start: 8, end: 12, start_anchor: 'before', end_anchor: 'after', attrs: {} },
  ];

  it('drops marks entirely before split', () => {
    const clipped = clipMarksForSuffix(marks, 6);
    const bold = clipped.find((m) => m.mark_type === 'bold');
    expect(bold).toBeUndefined();
  });

  it('clips marks spanning the split and starts at 0', () => {
    const clipped = clipMarksForSuffix(marks, 6);
    const italic = clipped.find((m) => m.mark_type === 'italic');
    expect(italic).toBeDefined();
    expect(italic?.start).toBe(0); // starts at 0 (was inside the mark)
    expect(italic?.end).toBe(4); // 10 - 6 = 4
  });

  it('shifts marks entirely after split', () => {
    const clipped = clipMarksForSuffix(marks, 6);
    const code = clipped.find((m) => m.mark_type === 'code');
    expect(code).toBeDefined();
    expect(code?.start).toBe(2); // 8 - 6 = 2
    expect(code?.end).toBe(6); // 12 - 6 = 6
  });
});

// =============================================================================
// Comment Adjustment Tests
// =============================================================================

describe('adjustCommentsForStartBlock', () => {
  const comments: Comment[] = [
    { id: 'c1', block_id: 'b1', start: 0, end: 3, content: 'Before', author: 'user', created_at: '', resolved: false },
    { id: 'c2', block_id: 'b1', start: 2, end: 8, content: 'Spans', author: 'user', created_at: '', resolved: false },
    { id: 'c3', block_id: 'b1', start: 6, end: 10, content: 'After', author: 'user', created_at: '', resolved: false },
    { id: 'c4', block_id: 'b2', start: 0, end: 5, content: 'Other block', author: 'user', created_at: '', resolved: false },
  ];

  it('keeps comments entirely before relStart', () => {
    const adjusted = adjustCommentsForStartBlock(comments, 'b1', 5);
    const c1 = adjusted.find((c) => c.id === 'c1');
    expect(c1).toBeDefined();
    expect(c1?.start).toBe(0);
    expect(c1?.end).toBe(3);
  });

  it('clips comments spanning relStart', () => {
    const adjusted = adjustCommentsForStartBlock(comments, 'b1', 5);
    const c2 = adjusted.find((c) => c.id === 'c2');
    expect(c2).toBeDefined();
    expect(c2?.start).toBe(2);
    expect(c2?.end).toBe(5); // clipped from 8
  });

  it('drops comments entirely in deleted portion', () => {
    const adjusted = adjustCommentsForStartBlock(comments, 'b1', 5);
    const c3 = adjusted.find((c) => c.id === 'c3');
    expect(c3).toBeUndefined();
  });

  it('passes through comments from other blocks', () => {
    const adjusted = adjustCommentsForStartBlock(comments, 'b1', 5);
    const c4 = adjusted.find((c) => c.id === 'c4');
    expect(c4).toBeDefined();
  });
});

describe('adjustCommentsForEndBlock', () => {
  const comments: Comment[] = [
    { id: 'c1', block_id: 'b1', start: 0, end: 3, content: 'Before', author: 'user', created_at: '', resolved: false },
    { id: 'c2', block_id: 'b1', start: 2, end: 8, content: 'Spans', author: 'user', created_at: '', resolved: false },
    { id: 'c3', block_id: 'b1', start: 6, end: 10, content: 'After', author: 'user', created_at: '', resolved: false },
  ];

  it('drops comments entirely before relEnd', () => {
    const adjusted = adjustCommentsForEndBlock(comments, 'b1', 5, 10);
    const c1 = adjusted.find((c) => c.id === 'c1');
    expect(c1).toBeUndefined();
  });

  it('clips and shifts comments spanning relEnd', () => {
    const adjusted = adjustCommentsForEndBlock(comments, 'b1', 5, 10);
    const c2 = adjusted.find((c) => c.id === 'c2');
    expect(c2).toBeDefined();
    expect(c2?.start).toBe(10); // newSuffixStart
    expect(c2?.end).toBe(13); // 8 - 5 + 10 = 13
  });

  it('shifts comments entirely after relEnd', () => {
    const adjusted = adjustCommentsForEndBlock(comments, 'b1', 5, 10);
    const c3 = adjusted.find((c) => c.id === 'c3');
    expect(c3).toBeDefined();
    expect(c3?.start).toBe(11); // 6 + (10 - 5) = 11
    expect(c3?.end).toBe(15); // 10 + (10 - 5) = 15
  });
});

describe('migrateCommentsFromDeletedBlocks', () => {
  const comments: Comment[] = [
    { id: 'c1', block_id: 'b1', start: 0, end: 5, content: 'Keep', author: 'user', created_at: '', resolved: false },
    { id: 'c2', block_id: 'b2', start: 3, end: 8, content: 'Migrate1', author: 'user', created_at: '', resolved: false },
    { id: 'c3', block_id: 'b3', start: 0, end: 2, content: 'Migrate2', author: 'user', created_at: '', resolved: false },
  ];

  it('migrates comments from deleted blocks to target', () => {
    const deletedIds = new Set(['b2', 'b3']);
    const migrated = migrateCommentsFromDeletedBlocks(comments, deletedIds, 'b1', 10);

    const c2 = migrated.find((c) => c.id === 'c2');
    expect(c2).toBeDefined();
    expect(c2?.block_id).toBe('b1');
    expect(c2?.start).toBe(10);
    expect(c2?.end).toBe(10); // collapsed to a point
    expect(c2?.migrated_from).toBe('b2');
  });

  it('keeps comments from non-deleted blocks', () => {
    const deletedIds = new Set(['b2', 'b3']);
    const migrated = migrateCommentsFromDeletedBlocks(comments, deletedIds, 'b1', 10);

    const c1 = migrated.find((c) => c.id === 'c1');
    expect(c1).toBeDefined();
    expect(c1?.block_id).toBe('b1');
    expect(c1?.start).toBe(0);
    expect(c1?.migrated_from).toBeUndefined();
  });
});

// =============================================================================
// Cross-Block Transform Tests
// =============================================================================

describe('planCrossBlockTransform', () => {
  it('plans a simple delete across two paragraphs', () => {
    // "Hello [world\n\nGoodbye] friend" -> "Hello  friend"
    const plan = planCrossBlockTransform(
      'Hello world',
      'Goodbye friend',
      6, // after "Hello "
      8, // after "Goodbye "
      '', // delete, no insertion
    );
    expect(plan.prefix).toBe('Hello ');
    expect(plan.suffix).toBe('friend');
    expect(plan.newBlocks).toHaveLength(0);
  });

  it('plans paste creating new paragraphs', () => {
    const plan = planCrossBlockTransform(
      'First paragraph',
      'Second paragraph',
      5, // after "First"
      7, // after "Second "
      '\n\nNew block\n\n',
    );
    expect(plan.prefix).toBe('First');
    expect(plan.suffix).toBe('paragraph');
    expect(plan.newBlocks.length).toBeGreaterThan(0);
  });

  it('preserves marks before selection in start block', () => {
    const startMarks: Mark[] = [
      { mark_type: 'bold', start: 0, end: 3, start_anchor: 'before', end_anchor: 'after', attrs: {} },
      { mark_type: 'italic', start: 5, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Hello world',
      'Goodbye',
      4,
      3,
      '',
      startMarks,
    );
    expect(plan.preservedStartMarks).toHaveLength(1);
    expect(plan.preservedStartMarks[0].mark_type).toBe('bold');
  });

  it('preserves and shifts marks after selection in end block', () => {
    const endMarks: Mark[] = [
      { mark_type: 'underline', start: 5, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Start',
      'End with formatting',
      3,
      4,
      '',
      [],
      endMarks,
    );
    expect(plan.preservedEndMarks).toHaveLength(1);
    expect(plan.preservedEndMarks[0].start).toBe(1); // shifted from 5
    expect(plan.preservedEndMarks[0].end).toBe(6); // shifted from 10
  });
});

describe('computeResultBlocks', () => {
  const startBlock: BlockData = {
    id: 'b1',
    kind: 'paragraph',
    text: 'Hello world',
  };

  it('merges prefix and suffix when no new blocks', () => {
    const plan = planCrossBlockTransform(
      'Hello world',
      'Goodbye friend',
      6,
      8,
      '',
    );
    const results = computeResultBlocks(plan, startBlock);
    expect(results).toHaveLength(1);
    expect(results[0].originalId).toBe('b1');
    expect(results[0].text).toBe('Hello friend');
  });

  it('merges single block of same kind', () => {
    const plan = planCrossBlockTransform(
      'Hello world',
      'Goodbye friend',
      6,
      8,
      'beautiful ',
    );
    const results = computeResultBlocks(plan, startBlock);
    expect(results).toHaveLength(1);
    expect(results[0].text).toBe('Hello beautiful friend');
  });

  it('creates new blocks for different kinds', () => {
    const plan = planCrossBlockTransform(
      'Hello world',
      'Goodbye friend',
      6,
      8,
      '\n\n# New Heading\n\n',
    );
    const results = computeResultBlocks(plan, startBlock);
    expect(results.length).toBeGreaterThan(1);
    // First block should be the modified start block with prefix
    expect(results[0].originalId).toBe('b1');
    expect(results[0].kind).toBe('paragraph');
    // Should have a heading somewhere
    const headingBlock = results.find((b) => b.kind === 'heading');
    expect(headingBlock).toBeDefined();
  });

  it('handles paste with code block', () => {
    const plan = planCrossBlockTransform(
      'Before code',
      'After code',
      7,
      6,
      '\n\n```js\ncode();\n```\n\n',
    );
    const results = computeResultBlocks(plan, startBlock);
    const codeBlock = results.find((b) => b.kind === 'code_block');
    expect(codeBlock).toBeDefined();
    expect(codeBlock?.text).toContain('code');
  });

  it('preserves marks through transformation', () => {
    const startMarks: Mark[] = [
      { mark_type: 'bold', start: 0, end: 3, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    // End marks must start AT or AFTER the split position (5) to be preserved
    // "Good friend" split at 5 -> suffix is "friend"
    // Italic on "friend" would be at [5, 11] in original, becomes [0, 6] after shift
    const endMarks: Mark[] = [
      { mark_type: 'italic', start: 5, end: 11, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Hello world',
      'Good friend',
      6, // prefix is "Hello "
      5, // suffix is "friend"
      '',
      startMarks,
      endMarks,
    );
    const results = computeResultBlocks(plan, startBlock);
    expect(results[0].marks).toHaveLength(2);
    // Bold should be at original position
    expect(results[0].marks[0].mark_type).toBe('bold');
    expect(results[0].marks[0].start).toBe(0);
    expect(results[0].marks[0].end).toBe(3);
    // Italic was at [5,11] in end block, after shifting becomes [0,6], then offset by prefix (6)
    expect(results[0].marks[1].mark_type).toBe('italic');
    expect(results[0].marks[1].start).toBe(6); // "Hello " = 6 chars
    expect(results[0].marks[1].end).toBe(12); // 6 + 6 = 12
  });

  it('clips marks spanning selection boundary (not drops)', () => {
    // Mark spans from inside prefix into deleted portion
    const startMarks: Mark[] = [
      { mark_type: 'bold', start: 2, end: 8, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    // Mark spans from deleted portion into suffix
    const endMarks: Mark[] = [
      { mark_type: 'italic', start: 2, end: 9, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Hello world',
      'Good friend',
      5, // prefix is "Hello"
      5, // suffix is "friend"
      '',
      startMarks,
      endMarks,
    );
    const results = computeResultBlocks(plan, startBlock);

    // Should have both marks, both clipped
    expect(results[0].marks).toHaveLength(2);

    // Bold was [2,8], clipped to [2,5] (prefix length)
    const bold = results[0].marks.find((m) => m.mark_type === 'bold');
    expect(bold).toBeDefined();
    expect(bold?.start).toBe(2);
    expect(bold?.end).toBe(5);

    // Italic was [2,9] in end block, clipped to [0,4] (9-5=4), then shifted by prefix (5)
    const italic = results[0].marks.find((m) => m.mark_type === 'italic');
    expect(italic).toBeDefined();
    expect(italic?.start).toBe(5); // 0 + 5 (prefix)
    expect(italic?.end).toBe(9); // 4 + 5 (prefix)
  });
});

// =============================================================================
// Integration Scenarios
// =============================================================================

describe('Cross-block edit scenarios', () => {
  it('handles multi-block delete (para→para)', () => {
    // Scenario: Select from middle of para1 through middle of para3
    // Para 1: "First paragraph text"
    // Para 2: "Middle paragraph" (fully selected)
    // Para 3: "Last paragraph text"
    // Selection: "[raph text\n\nMiddle paragraph\n\nLast para]"
    const plan = planCrossBlockTransform(
      'First paragraph text',
      'Last paragraph text',
      10, // after "First para"
      9, // after "Last para"
      '',
      [],
      [],
      ['b2'], // middle block to delete
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'First paragraph text' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].text).toBe('First paragraph text');
    expect(plan.deleteBlockIds).toContain('b2');
  });

  it('handles paste creating new paragraphs into selection', () => {
    // Scenario: Select across two blocks and paste multi-paragraph content
    const plan = planCrossBlockTransform(
      'Intro text here',
      'Outro text there',
      6, // after "Intro "
      6, // after "Outro "
      'new first para\n\nnew second para',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Intro text here' };
    const results = computeResultBlocks(plan, startBlock);

    // Should have: modified b1, new para, modified suffix
    expect(results.length).toBeGreaterThanOrEqual(2);
    expect(results[0].text).toContain('Intro ');
    expect(results[0].text).toContain('new first para');
  });

  it('handles edits inside code blocks across boundaries', () => {
    // Scenario: Selection starts in a code block and ends in another
    const plan = planCrossBlockTransform(
      'function foo() {\n  return 1;\n}',
      'function bar() {\n  return 2;\n}',
      20, // somewhere in middle
      10,
      '', // delete
    );
    const startBlock: BlockData = { id: 'c1', kind: 'code_block', text: 'function foo()...' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results[0].kind).toBe('code_block');
  });

  it('handles list item edits across boundaries', () => {
    const plan = planCrossBlockTransform(
      'First list item',
      'Second list item',
      6,
      7,
      '',
    );
    const startBlock: BlockData = { id: 'li1', kind: 'list_item', text: 'First list item' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results[0].kind).toBe('list_item');
    expect(results[0].text).toBe('First list item');
  });
});

// =============================================================================
// Advanced Reflow Tests
// =============================================================================

describe('Block reflow edge cases', () => {
  it('handles empty prefix with paragraph insert', () => {
    // Selection starts at beginning of block
    const plan = planCrossBlockTransform(
      'First block',
      'Second block',
      0, // empty prefix
      6, // "Second"
      'New text',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'First block' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].text).toBe('New text block');
    expect(results[0].originalId).toBe('b1');
  });

  it('handles empty suffix with paragraph insert', () => {
    // Selection ends at end of block
    const plan = planCrossBlockTransform(
      'First block',
      'Second block',
      6, // "First "
      12, // end of "Second block"
      'New text',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'First block' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].text).toBe('First New text');
  });

  it('handles empty prefix AND suffix (full replacement)', () => {
    const plan = planCrossBlockTransform(
      'First block',
      'Second block',
      0,
      12,
      'Completely new',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'First block' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].text).toBe('Completely new');
  });

  it('handles inserting heading into paragraph', () => {
    const plan = planCrossBlockTransform(
      'Some text here',
      'More text there',
      5, // "Some "
      5, // "More "
      '# New Heading\n\n',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Some text here' };
    const results = computeResultBlocks(plan, startBlock);

    // Should have: paragraph with "Some ", heading "New Heading", paragraph with "text there"
    expect(results.length).toBeGreaterThanOrEqual(2);
    expect(results[0].kind).toBe('paragraph');
    expect(results[0].text).toBe('Some ');

    const headingBlock = results.find((r) => r.kind === 'heading');
    expect(headingBlock).toBeDefined();
  });

  it('handles inserting code block into paragraph', () => {
    const plan = planCrossBlockTransform(
      'Before code',
      'After code',
      7, // "Before "
      6, // "After "
      '```js\nconst x = 1;\n```\n\n',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Before code' };
    const results = computeResultBlocks(plan, startBlock);

    const codeBlock = results.find((r) => r.kind === 'code_block');
    expect(codeBlock).toBeDefined();
    expect(codeBlock?.text).toContain('const x = 1');
  });

  it('handles inserting list items into paragraph', () => {
    const plan = planCrossBlockTransform(
      'Intro text',
      'Outro text',
      6, // "Intro "
      6, // "Outro "
      '- Item 1\n- Item 2\n',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Intro text' };
    const results = computeResultBlocks(plan, startBlock);

    const listItems = results.filter((r) => r.kind === 'list_item');
    expect(listItems.length).toBeGreaterThanOrEqual(1);
  });

  it('handles mixed block types in insert', () => {
    const plan = planCrossBlockTransform(
      'Start here',
      'End there',
      6, // "Start "
      4, // "End "
      '# Heading\n\nParagraph\n\n```\ncode\n```\n\n',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Start here' };
    const results = computeResultBlocks(plan, startBlock);

    const kinds = results.map((r) => r.kind);
    expect(kinds).toContain('heading');
    expect(kinds).toContain('code_block');
  });

  it('preserves start block when first insert has different kind', () => {
    const plan = planCrossBlockTransform(
      'Keep this prefix',
      'And this suffix',
      16, // "Keep this prefix"
      0, // full suffix
      '# Heading\n\n',
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Keep this prefix' };
    const results = computeResultBlocks(plan, startBlock);

    // Start block should keep prefix
    expect(results[0].originalId).toBe('b1');
    expect(results[0].kind).toBe('paragraph');
    expect(results[0].text).toBe('Keep this prefix');
  });
});

describe('Marks reflow across blocks', () => {
  it('carries suffix marks to last result block', () => {
    const startMarks: Mark[] = [
      { mark_type: 'bold', start: 0, end: 4, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const endMarks: Mark[] = [
      { mark_type: 'italic', start: 5, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Bold text here',
      'Some italic end',
      5, // "Bold "
      5, // "Some "
      '# Heading\n\n',
      startMarks,
      endMarks,
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Bold text here' };
    const results = computeResultBlocks(plan, startBlock);

    // Last block should have the suffix marks (shifted)
    const lastBlock = results[results.length - 1];
    expect(lastBlock.marks.length).toBeGreaterThan(0);
    expect(lastBlock.marks[0].mark_type).toBe('italic');
  });

  it('preserves marks on start block prefix', () => {
    const startMarks: Mark[] = [
      { mark_type: 'bold', start: 0, end: 4, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    ];
    const plan = planCrossBlockTransform(
      'Bold rest of text',
      'End text',
      5, // "Bold "
      4,
      '# Heading\n\n',
      startMarks,
    );
    const startBlock: BlockData = { id: 'b1', kind: 'paragraph', text: 'Bold rest of text' };
    const results = computeResultBlocks(plan, startBlock);

    // First block should still have bold mark (clipped to prefix)
    expect(results[0].marks.length).toBe(1);
    expect(results[0].marks[0].mark_type).toBe('bold');
    expect(results[0].marks[0].end).toBe(4); // clipped to prefix length
  });
});

describe('List block specific scenarios', () => {
  it('merges list items when both blocks are list_item', () => {
    const plan = planCrossBlockTransform(
      'First item content',
      'Second item content',
      6, // "First "
      7, // "Second "
      '',
    );
    const startBlock: BlockData = { id: 'li1', kind: 'list_item', text: 'First item content' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].kind).toBe('list_item');
    expect(results[0].text).toBe('First item content');
  });

  it('handles paste of list into list item', () => {
    const plan = planCrossBlockTransform(
      'First item',
      'Second item',
      6, // "First "
      7, // "Second "
      '- New item 1\n- New item 2\n',
    );
    const startBlock: BlockData = { id: 'li1', kind: 'list_item', text: 'First item' };
    const results = computeResultBlocks(plan, startBlock);

    // Should have multiple list items
    const listItems = results.filter((r) => r.kind === 'list_item');
    expect(listItems.length).toBeGreaterThanOrEqual(2);
  });
});

describe('Code block specific scenarios', () => {
  it('merges code blocks when both are code_block', () => {
    const plan = planCrossBlockTransform(
      'function a() {}',
      'function b() {}',
      11, // "function a("
      11, // "function b("
      '',
    );
    const startBlock: BlockData = { id: 'cb1', kind: 'code_block', attrs: { language: 'js' }, text: 'function a() {}' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].kind).toBe('code_block');
    expect(results[0].text).toBe('function a() {}');
  });

  it('handles paste of code into code block', () => {
    const plan = planCrossBlockTransform(
      'const a = 1;',
      'const b = 2;',
      12,
      0,
      '\nconst c = 3;\n',
    );
    const startBlock: BlockData = { id: 'cb1', kind: 'code_block', text: 'const a = 1;' };
    const results = computeResultBlocks(plan, startBlock);

    // Insert is plain text (paragraph), should create new structure
    expect(results[0].kind).toBe('code_block');
  });
});

describe('Heading specific scenarios', () => {
  it('handles heading to heading merge', () => {
    const plan = planCrossBlockTransform(
      'First Heading',
      'Second Heading',
      6, // "First "
      7, // "Second "
      '',
    );
    const startBlock: BlockData = { id: 'h1', kind: 'heading', attrs: { level: 1 }, text: 'First Heading' };
    const results = computeResultBlocks(plan, startBlock);

    expect(results).toHaveLength(1);
    expect(results[0].kind).toBe('heading');
    expect(results[0].text).toBe('First Heading');
  });

  it('handles inserting heading into heading (merges same kind)', () => {
    const plan = planCrossBlockTransform(
      'Main Title',
      'Subtitle',
      5, // "Main "
      3, // "Sub"
      '## New Section\n\n',
    );
    const startBlock: BlockData = { id: 'h1', kind: 'heading', attrs: { level: 1 }, text: 'Main Title' };
    const results = computeResultBlocks(plan, startBlock);

    // Same-kind blocks merge: heading + heading -> single heading
    // prefix "Main " + parsed "New Section" + suffix "title"
    expect(results[0].kind).toBe('heading');
    expect(results[0].text).toBe('Main New Sectiontitle');
    expect(results[0].originalId).toBe('h1');
  });

  it('handles inserting paragraph into heading (keeps separate)', () => {
    const plan = planCrossBlockTransform(
      'Main Title',
      'Subtitle',
      5, // "Main "
      3, // "Sub"
      'some paragraph text\n\n',
    );
    const startBlock: BlockData = { id: 'h1', kind: 'heading', attrs: { level: 1 }, text: 'Main Title' };
    const results = computeResultBlocks(plan, startBlock);

    // Different kinds stay separate: heading prefix, then paragraph with suffix
    expect(results[0].kind).toBe('heading');
    expect(results[0].text).toBe('Main ');
    expect(results.length).toBeGreaterThan(1);
  });
});
