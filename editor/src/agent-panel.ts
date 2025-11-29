/**
 * Agent panel for collaborative AI editing.
 *
 * Provides a chat-like interface for interacting with the agent,
 * with support for selection-based queries and streaming responses.
 */

import { CollabAPI, Selection, AgentStreamEvent } from './api';

export interface AgentPanelOptions {
  container: HTMLElement;
  api: CollabAPI;
  getSelection: () => Selection | null;
  getCurrentSlug: () => string | null;
}

export class AgentPanel {
  private container: HTMLElement;
  private api: CollabAPI;
  private getSelection: () => Selection | null;
  private getCurrentSlug: () => string | null;

  private panel: HTMLElement;
  private messagesContainer: HTMLElement;
  private inputArea: HTMLTextAreaElement;
  private sendButton: HTMLButtonElement;
  private isVisible = false;
  private isLoading = false;

  constructor(options: AgentPanelOptions) {
    this.container = options.container;
    this.api = options.api;
    this.getSelection = options.getSelection;
    this.getCurrentSlug = options.getCurrentSlug;

    this.panel = this.createPanel();
    this.messagesContainer = this.panel.querySelector('.agent-messages')!;
    this.inputArea = this.panel.querySelector('.agent-input')!;
    this.sendButton = this.panel.querySelector('.agent-send')!;

    this.setupEventListeners();
    this.container.appendChild(this.panel);
  }

  private createPanel(): HTMLElement {
    const panel = document.createElement('div');
    panel.className = 'agent-panel';
    panel.innerHTML = `
      <div class="agent-header">
        <span class="agent-title">AI Assistant</span>
        <button class="agent-close" title="Close (Escape)">×</button>
      </div>
      <div class="agent-messages"></div>
      <div class="agent-selection-preview"></div>
      <div class="agent-input-area">
        <textarea class="agent-input" placeholder="Ask the agent... (Cmd+Enter to send)" rows="2"></textarea>
        <button class="agent-send">Send</button>
      </div>
    `;
    return panel;
  }

  private setupEventListeners() {
    // Close button
    this.panel.querySelector('.agent-close')!.addEventListener('click', () => {
      this.hide();
    });

    // Send button
    this.sendButton.addEventListener('click', () => {
      this.send();
    });

    // Keyboard shortcuts
    this.inputArea.addEventListener('keydown', (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
        e.preventDefault();
        this.send();
      }
    });

    // Global keyboard shortcut to toggle panel
    document.addEventListener('keydown', (e) => {
      // Cmd+K or Ctrl+K to toggle
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        this.toggle();
      }
      // Escape to close
      if (e.key === 'Escape' && this.isVisible) {
        this.hide();
      }
    });
  }

  toggle() {
    if (this.isVisible) {
      this.hide();
    } else {
      this.show();
    }
  }

  show() {
    this.panel.classList.add('visible');
    this.isVisible = true;
    this.updateSelectionPreview();
    this.inputArea.focus();
  }

  hide() {
    this.panel.classList.remove('visible');
    this.isVisible = false;
  }

  private updateSelectionPreview() {
    const preview = this.panel.querySelector('.agent-selection-preview')!;
    const selection = this.getSelection();

    if (selection && selection.text) {
      const truncated = selection.text.length > 100
        ? selection.text.substring(0, 100) + '...'
        : selection.text;
      preview.innerHTML = `<div class="selection-badge">Selected: "${truncated}"</div>`;
    } else {
      preview.innerHTML = '';
    }
  }

  private async send() {
    const query = this.inputArea.value.trim();
    if (!query || this.isLoading) return;

    const slug = this.getCurrentSlug();
    if (!slug) {
      this.addMessage('system', 'Please open a document first.');
      return;
    }

    const selection = this.getSelection();

    // Add user message
    this.addMessage('user', query);
    this.inputArea.value = '';
    this.inputArea.disabled = true;
    this.sendButton.disabled = true;
    this.isLoading = true;

    // Add thinking indicator
    const thinkingEl = this.addMessage('assistant', '');
    thinkingEl.classList.add('thinking');
    thinkingEl.innerHTML = '<span class="thinking-dots">Thinking</span>';

    try {
      const response = await this.api.askAgent({
        query,
        slug,
        selection: selection || undefined,
      });

      // Remove thinking indicator and show response
      thinkingEl.classList.remove('thinking');
      thinkingEl.textContent = response.response;

      if (response.made_edits) {
        this.addMessage('system', 'The document was updated.');
      }

    } catch (err) {
      thinkingEl.classList.remove('thinking');
      thinkingEl.classList.add('error');
      thinkingEl.textContent = `Error: ${err}`;
    } finally {
      this.inputArea.disabled = false;
      this.sendButton.disabled = false;
      this.isLoading = false;
      this.inputArea.focus();
    }
  }

  private addMessage(role: 'user' | 'assistant' | 'system', content: string): HTMLElement {
    const msg = document.createElement('div');
    msg.className = `agent-message ${role}`;
    msg.textContent = content;
    this.messagesContainer.appendChild(msg);
    this.messagesContainer.scrollTop = this.messagesContainer.scrollHeight;
    return msg;
  }

  /**
   * Send a request with streaming response via WebSocket.
   * For longer interactions, this provides better UX.
   */
  async sendStreaming(query: string): Promise<void> {
    const slug = this.getCurrentSlug();
    if (!slug) return;

    const selection = this.getSelection();
    const sessionId = `${Date.now()}-${Math.random().toString(36).slice(2)}`;

    // Add user message
    this.addMessage('user', query);

    // Add assistant message container
    const assistantMsg = this.addMessage('assistant', '');
    assistantMsg.classList.add('streaming');

    const ws = this.api.createAgentSocket(sessionId);

    return new Promise((resolve, reject) => {
      let responseText = '';

      ws.onopen = () => {
        // Send the request
        ws.send(JSON.stringify({
          query,
          slug,
          selection: selection || undefined,
        }));
      };

      ws.onmessage = (event) => {
        const data: AgentStreamEvent = JSON.parse(event.data);

        switch (data.type) {
          case 'thinking':
            assistantMsg.innerHTML = '<span class="thinking-dots">Thinking</span>';
            break;
          case 'text':
            responseText += data.content;
            assistantMsg.textContent = responseText;
            break;
          case 'tool_call':
            // Could show tool usage indicator
            break;
          case 'tool_result':
            // Could show result indicator
            break;
          case 'done':
            assistantMsg.classList.remove('streaming');
            ws.close();
            resolve();
            break;
          case 'error':
            assistantMsg.classList.remove('streaming');
            assistantMsg.classList.add('error');
            assistantMsg.textContent = `Error: ${data.message}`;
            ws.close();
            reject(new Error(data.message));
            break;
        }

        this.messagesContainer.scrollTop = this.messagesContainer.scrollHeight;
      };

      ws.onerror = (err) => {
        assistantMsg.classList.remove('streaming');
        assistantMsg.classList.add('error');
        assistantMsg.textContent = 'Connection error';
        reject(err);
      };

      ws.onclose = () => {
        assistantMsg.classList.remove('streaming');
      };
    });
  }
}

// CSS for the agent panel (inject into document)
const agentStyles = `
.agent-panel {
  position: fixed;
  right: -400px;
  top: 0;
  width: 400px;
  height: 100vh;
  background: var(--bg-secondary, #1e1e1e);
  border-left: 1px solid var(--border-color, #333);
  display: flex;
  flex-direction: column;
  transition: right 0.2s ease;
  z-index: 1000;
  font-family: system-ui, -apple-system, sans-serif;
}

.agent-panel.visible {
  right: 0;
}

.agent-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color, #333);
  background: var(--bg-tertiary, #252525);
}

.agent-title {
  font-weight: 600;
  color: var(--text-primary, #fff);
}

.agent-close {
  background: none;
  border: none;
  color: var(--text-secondary, #888);
  font-size: 20px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
}

.agent-close:hover {
  background: var(--bg-hover, #333);
  color: var(--text-primary, #fff);
}

.agent-messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.agent-message {
  padding: 10px 14px;
  border-radius: 8px;
  max-width: 90%;
  word-wrap: break-word;
  line-height: 1.5;
}

.agent-message.user {
  background: var(--accent-color, #0066cc);
  color: white;
  align-self: flex-end;
}

.agent-message.assistant {
  background: var(--bg-tertiary, #2d2d2d);
  color: var(--text-primary, #e0e0e0);
  align-self: flex-start;
}

.agent-message.system {
  background: transparent;
  color: var(--text-secondary, #888);
  font-size: 0.9em;
  align-self: center;
  font-style: italic;
}

.agent-message.error {
  background: #4a1c1c;
  color: #ff6b6b;
}

.agent-message.thinking .thinking-dots::after {
  content: '';
  animation: thinking 1.5s infinite;
}

@keyframes thinking {
  0% { content: ''; }
  25% { content: '.'; }
  50% { content: '..'; }
  75% { content: '...'; }
}

.agent-selection-preview {
  padding: 0 16px;
}

.selection-badge {
  background: var(--bg-tertiary, #2d2d2d);
  border: 1px solid var(--border-color, #444);
  border-radius: 4px;
  padding: 8px 12px;
  font-size: 0.85em;
  color: var(--text-secondary, #aaa);
  margin-bottom: 8px;
}

.agent-input-area {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-color, #333);
  background: var(--bg-tertiary, #252525);
}

.agent-input {
  flex: 1;
  background: var(--bg-primary, #1a1a1a);
  border: 1px solid var(--border-color, #444);
  border-radius: 6px;
  padding: 10px 12px;
  color: var(--text-primary, #e0e0e0);
  font-size: 14px;
  resize: none;
  font-family: inherit;
}

.agent-input:focus {
  outline: none;
  border-color: var(--accent-color, #0066cc);
}

.agent-input:disabled {
  opacity: 0.6;
}

.agent-send {
  background: var(--accent-color, #0066cc);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 10px 16px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.agent-send:hover:not(:disabled) {
  background: var(--accent-hover, #0077ee);
}

.agent-send:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
`;

// Inject styles
const styleEl = document.createElement('style');
styleEl.textContent = agentStyles;
document.head.appendChild(styleEl);
