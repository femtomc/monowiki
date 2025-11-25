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
}
