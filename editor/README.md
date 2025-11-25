# monowiki-editor

Collaborative markdown editor for monowiki using CodeMirror 6 and Yjs.

## Setup

```bash
bun install
```

## Development

Start the editor dev server (proxies to collab daemon):

```bash
bun run dev
```

This starts Vite on port 5173 with proxy configuration:
- `/api/*` → `http://localhost:8787` (collab daemon)
- `/ws/*` → `ws://localhost:8787` (collab daemon WebSocket)

**Before using the editor**, ensure you have:

1. **Collab daemon running** on port 8787:
   ```bash
   cargo run -p monowiki-collab -- \
     --repo <your-repo-url> \
     --branch main \
     --listen-addr 0.0.0.0:8787
   ```

2. **Dev server running** (for preview iframe):
   ```bash
   monowiki dev --port 3000
   ```

3. Open `http://localhost:5173` in your browser

## Usage

1. Enter a note slug (e.g., `hello-world` or `drafts/my-note`)
2. Click "Open" to connect to the collaborative doc
3. Edit in the left pane; see rendered preview in the right pane
4. Use "Checkpoint" to commit and push changes to git
5. Use "Build" to rebuild the site (updates preview)

## Build for production

```bash
bun run build
```

Output goes to `dist/`.
