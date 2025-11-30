/**
 * HTTP API client for the monowiki-collab daemon.
 */

export interface NoteResponse {
  slug: string;
  path: string;
  frontmatter: Record<string, unknown>;
  body: string;
}

export interface CheckpointResponse {
  committed: boolean;
  message: string;
}

export interface FlushResponse {
  message: string;
}

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children?: FileEntry[];
}

export interface FilesResponse {
  files: FileEntry[];
}

// Agent types
export interface Selection {
  text: string;
  block_id: string;
  start: number;
  end: number;
}

export interface AskRequest {
  query: string;
  slug: string;
  selection?: Selection;
}

export interface AskResponse {
  response: string;
  made_edits: boolean;
}

export interface Comment {
  id: string;
  block_id: string;
  start: number;
  end: number;
  content: string;
  author: string;
  created_at: string;
  resolved: boolean;
  migrated_from?: string;
}

export interface CommentsResponse {
  comments: Comment[];
}

export interface GraphResponse {
  slug: string;
  backlinks: string[];
  outgoing: string[];
}

export interface SearchHit {
  id: string;
  slug: string;
  title: string;
  section_title: string;
  snippet: string;
  url: string;
  tags: string[];
  doc_type: string;
}

export interface SearchResponse {
  results: SearchHit[];
}

// Dataspace events (as pulled via graph/search endpoints for now)
export interface DocEvent {
  event: string;
  slug: string;
  [key: string]: any;
}

export interface EventsResponse {
  events: string[];
}

// Agent streaming events
export type AgentStreamEvent =
  | { type: 'thinking' }
  | { type: 'text'; content: string }
  | { type: 'tool_call'; name: string }
  | { type: 'tool_result'; name: string; success: boolean }
  | { type: 'done'; response: string }
  | { type: 'error'; message: string };

export class CollabAPI {
  private baseUrl: string;
  private token: string | null;

  constructor(baseUrl: string = '', token: string | null = null) {
    this.baseUrl = baseUrl;
    this.token = token;
  }

  setToken(token: string | null) {
    this.token = token;
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = {
      'Content-Type': 'application/json',
    };
    if (this.token) {
      h['Authorization'] = `Bearer ${this.token}`;
    }
    return h;
  }

  async getNote(slug: string): Promise<NoteResponse> {
    const res = await fetch(`${this.baseUrl}/api/note/${slug}`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Failed to get note: ${res.status} ${res.statusText}`);
    }
    return res.json();
  }

  async writeNote(
    slug: string,
    body: string,
    frontmatter?: Record<string, unknown>,
    checkpoint?: boolean
  ): Promise<{ path: string; checkpointed: boolean }> {
    const res = await fetch(`${this.baseUrl}/api/note/${slug}`, {
      method: 'PUT',
      headers: this.headers(),
      body: JSON.stringify({ body, frontmatter, checkpoint }),
    });
    if (!res.ok) {
      throw new Error(`Failed to write note: ${res.status} ${res.statusText}`);
    }
    return res.json();
  }

  async checkpoint(): Promise<CheckpointResponse> {
    const res = await fetch(`${this.baseUrl}/api/checkpoint`, {
      method: 'POST',
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Checkpoint failed: ${res.status} ${res.statusText}`);
    }
    return res.json();
  }

  async build(): Promise<string> {
    const res = await fetch(`${this.baseUrl}/api/build`, {
      method: 'POST',
      headers: this.headers(),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Build failed: ${res.status} ${text}`);
    }
    return res.text();
  }

  async flush(): Promise<FlushResponse> {
    const res = await fetch(`${this.baseUrl}/api/flush`, {
      method: 'POST',
      headers: this.headers(),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Flush failed: ${res.status} ${text}`);
    }
    return res.json();
  }

  async status(): Promise<Record<string, unknown>> {
    const res = await fetch(`${this.baseUrl}/api/status`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Status check failed: ${res.status}`);
    }
    return res.json();
  }

  async listFiles(): Promise<FilesResponse> {
    const res = await fetch(`${this.baseUrl}/api/files`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Failed to list files: ${res.status}`);
    }
    return res.json();
  }

  /**
   * Incrementally render a single note without full rebuild.
   * Updates the HTML output for the given slug.
   */
  async render(slug: string): Promise<{ slug: string; success: boolean }> {
    const res = await fetch(`${this.baseUrl}/api/render/${slug}`, {
      method: 'POST',
      headers: this.headers(),
    });
    if (!res.ok) {
      const json = await res.json().catch(() => ({}));
      throw new Error(json.error || `Render failed: ${res.status}`);
    }
    return res.json();
  }

  /**
   * Build the WebSocket base URL (without slug).
   * y-websocket will append the room name (slug) to this.
   */
  wsBaseUrl(): string {
    if (this.baseUrl.startsWith('http')) {
      const url = new URL(this.baseUrl);
      const protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
      return `${protocol}//${url.host}/ws/note`;
    }
    // Relative URL - use current page's origin
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/ws/note`;
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Agent API
  // ─────────────────────────────────────────────────────────────────────────────

  /**
   * Ask the agent a question about the current document.
   */
  async askAgent(request: AskRequest): Promise<AskResponse> {
    const res = await fetch(`${this.baseUrl}/api/agent/ask`, {
      method: 'POST',
      headers: this.headers(),
      body: JSON.stringify(request),
    });
    if (!res.ok) {
      const json = await res.json().catch(() => ({}));
      throw new Error(json.error || `Agent request failed: ${res.status}`);
    }
    return res.json();
  }

  /**
   * Get comments on a document.
   */
  async getComments(slug: string): Promise<CommentsResponse> {
    const res = await fetch(`${this.baseUrl}/api/agent/comments/${slug}`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Failed to get comments: ${res.status}`);
    }
    return res.json();
  }

  /**
   * Resolve a comment.
   */
  async resolveComment(slug: string, commentId: string): Promise<void> {
    const res = await fetch(`${this.baseUrl}/api/agent/comments/${slug}/${commentId}/resolve`, {
      method: 'POST',
      headers: this.headers(),
    });
    if (!res.ok) {
      const json = await res.json().catch(() => ({}));
      throw new Error(json.error || `Failed to resolve comment: ${res.status}`);
    }
  }

  /**
   * Add a comment to a block range.
   */
  async addComment(
    slug: string,
    payload: { block_id: string; start: number; end: number; content: string; author?: string },
  ): Promise<{ id: string }> {
    const res = await fetch(`${this.baseUrl}/api/agent/comments/${slug}`, {
      method: 'POST',
      headers: this.headers(),
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      const json = await res.json().catch(() => ({}));
      throw new Error(json.error || `Failed to add comment: ${res.status}`);
    }
    return res.json();
  }

  /**
   * Create a WebSocket connection for streaming agent responses.
   */
  createAgentSocket(sessionId: string): WebSocket {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = this.baseUrl ? new URL(this.baseUrl).host : window.location.host;
    const url = `${protocol}//${host}/ws/agent/${sessionId}`;

    const ws = new WebSocket(url);
    return ws;
  }

  async getGraph(slug: string): Promise<GraphResponse> {
    const res = await fetch(`${this.baseUrl}/api/graph/${slug}`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Failed to fetch graph: ${res.status}`);
    }
    return res.json();
  }

  async search(query: string, limit = 10): Promise<SearchResponse> {
    const res = await fetch(
      `${this.baseUrl}/api/search?q=${encodeURIComponent(query)}&limit=${limit}`,
      { headers: this.headers() },
    );
    if (!res.ok) {
      throw new Error(`Search failed: ${res.status}`);
    }
    return res.json();
  }

  async getEvents(slug: string): Promise<EventsResponse> {
    const res = await fetch(`${this.baseUrl}/api/events/${slug}`, {
      headers: this.headers(),
    });
    if (!res.ok) {
      throw new Error(`Failed to fetch events: ${res.status}`);
    }
    return res.json();
  }
}
