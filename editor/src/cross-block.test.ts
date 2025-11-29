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
  mergeMarks,
  planCrossBlockTransform,
  computeResultBlocks,
  type Mark,
  type BlockData,
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

describe('filterMarksBeforeSplit', () => {
  const marks: Mark[] = [
    { mark_type: 'bold', start: 0, end: 5, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'italic', start: 3, end: 10, start_anchor: 'before', end_anchor: 'after', attrs: {} },
    { mark_type: 'code', start: 8, end: 12, start_anchor: 'before', end_anchor: 'after', attrs: {} },
  ];

  it('keeps marks that end before or at split position', () => {
    const filtered = filterMarksBeforeSplit(marks, 5);
    expect(filtered).toHaveLength(1);
    expect(filtered[0].mark_type).toBe('bold');
  });

  it('excludes marks that extend past split position', () => {
    const filtered = filterMarksBeforeSplit(marks, 3);
    expect(filtered).toHaveLength(0);
  });

  it('includes marks ending exactly at split', () => {
    const filtered = filterMarksBeforeSplit(marks, 10);
    expect(filtered).toHaveLength(2);
  });
});

describe('filterMarksAfterSplit', () => {
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

  it('excludes marks starting before split', () => {
    const filtered = filterMarksAfterSplit(marks, 8);
    expect(filtered).toHaveLength(1);
    expect(filtered[0].mark_type).toBe('code');
    expect(filtered[0].start).toBe(0);
    expect(filtered[0].end).toBe(4);
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
