/**
 * CodeMirror 6 editor with Yjs binding via y-websocket.
 */

import { EditorState } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from '@codemirror/language';
import { autocompletion } from '@codemirror/autocomplete';
import * as Y from 'yjs';
import { WebsocketProvider } from 'y-websocket';
import { yCollab } from 'y-codemirror.next';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected';

export interface EditorOptions {
  container: HTMLElement;
  wsBaseUrl: string;
  room: string;
  token?: string;
  onStatusChange?: (status: ConnectionStatus) => void;
  onContentChange?: () => void;
}

export interface EditorInstance {
  view: EditorView;
  doc: Y.Doc;
  provider: WebsocketProvider;
  destroy: () => void;
}

/**
 * Create a CodeMirror 6 editor with Yjs/y-websocket binding.
 */
export function createEditor(options: EditorOptions): EditorInstance {
  const { container, wsBaseUrl, room, token, onStatusChange, onContentChange } = options;

  // Create Yjs document
  const ydoc = new Y.Doc();
  const ytext = ydoc.getText('body');

  // Observe text changes and notify caller
  if (onContentChange) {
    ytext.observe(() => {
      onContentChange();
    });
  }

  // Connect to the collab server via WebSocket
  // y-websocket constructs URL as: `${wsBaseUrl}/${room}`
  const provider = new WebsocketProvider(
    wsBaseUrl,
    room,
    ydoc,
    {
      disableBc: true, // Disable broadcast channel (not needed for server-based sync)
      params: token ? { token } : undefined,
    }
  );

  // Track connection status
  const updateStatus = (status: ConnectionStatus) => {
    onStatusChange?.(status);
  };

  provider.on('status', (event: { status: string }) => {
    if (event.status === 'connected') {
      updateStatus('connected');
    } else if (event.status === 'connecting') {
      updateStatus('connecting');
    } else {
      updateStatus('disconnected');
    }
  });

  // Initial status
  updateStatus('connecting');

  // Create awareness for cursor sharing
  const awareness = provider.awareness;

  // Generate a random color for this user
  const userColor = randomColor();
  awareness.setLocalStateField('user', {
    name: 'Anonymous',
    color: userColor,
    colorLight: userColor + '33',
  });

  // Create CodeMirror state with extensions
  const state = EditorState.create({
    doc: ytext.toString(),
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

      // Yjs collaborative editing
      yCollab(ytext, awareness),

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

  // Create the editor view
  const view = new EditorView({
    state,
    parent: container,
  });

  // Cleanup function
  const destroy = () => {
    provider.destroy();
    ydoc.destroy();
    view.destroy();
  };

  return { view, doc: ydoc, provider, destroy };
}

/**
 * Generate a random hex color for user identification.
 */
function randomColor(): string {
  const colors = [
    '#f44336', '#e91e63', '#9c27b0', '#673ab7',
    '#3f51b5', '#2196f3', '#03a9f4', '#00bcd4',
    '#009688', '#4caf50', '#8bc34a', '#cddc39',
    '#ffeb3b', '#ffc107', '#ff9800', '#ff5722',
  ];
  return colors[Math.floor(Math.random() * colors.length)];
}
