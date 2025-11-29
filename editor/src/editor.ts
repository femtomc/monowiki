/**
 * Lightweight CodeMirror setup used with the Loro-backed collab client.
 */

import { EditorState } from '@codemirror/state';
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from '@codemirror/language';
import { autocompletion } from '@codemirror/autocomplete';
import type { ViewUpdate } from '@codemirror/view';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

export interface EditorOptions {
  container: HTMLElement;
  initialDoc?: string;
  onContentChange?: (update: ViewUpdate) => void;
}

export interface EditorInstance {
  view: EditorView;
  destroy: () => void;
}

/**
 * Create a standalone CodeMirror editor. Collaboration is handled outside via Loro.
 */
export function createEditor(options: EditorOptions): EditorInstance {
  const { container, initialDoc = '', onContentChange } = options;

  const state = EditorState.create({
    doc: initialDoc,
    extensions: [
      // Basic editor features
      lineNumbers(),
      highlightActiveLine(),
      highlightActiveLineGutter(),
      EditorView.lineWrapping,
      history(),
      bracketMatching(),
      autocompletion(),

      // Keymaps
      keymap.of([...defaultKeymap, ...historyKeymap]),

      // Markdown syntax highlighting
      markdown(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),

      // Change listener
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onContentChange?.(update);
        }
      }),

      // Theme - dark mode compatible
      EditorView.theme({
        '&': {
          height: '100%',
        },
        '.cm-content': {
          caretColor: '#cccccc',
        },
        '.cm-line': {
          padding: '0 8px',
        },
      }),
    ],
  });

  const view = new EditorView({
    state,
    parent: container,
  });

  const destroy = () => {
    view.destroy();
  };

  return { view, destroy };
}
