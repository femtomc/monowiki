use std::path::PathBuf;

use clap::Parser;

/// CLI for the collab/git bridge. Keeps the existing monowiki toolchain intact.
#[derive(Debug, Clone, Parser)]
#[command(name = "monowiki-collab", about = "Realtime collab + git checkpoint daemon for monowiki")]
pub struct Cli {
    /// Git repository URL or local path
    #[arg(long, env = "MONOWIKI_REPO", default_value = ".")]
    pub repo: String,

    /// Content branch to read/write
    #[arg(long, env = "MONOWIKI_BRANCH", default_value = "main")]
    pub branch: String,

    /// Optional deploy branch for built assets (e.g., gh-pages)
    #[arg(long, env = "MONOWIKI_DEPLOY_BRANCH")]
    pub deploy_branch: Option<String>,

    /// Listen address for HTTP/WS endpoints
    #[arg(long, env = "MONOWIKI_COLLAB_ADDR", default_value = "127.0.0.1:8787")]
    pub listen_addr: String,

    /// Working directory for cloned repo, temp state, and build artifacts
    #[arg(long, env = "MONOWIKI_WORKDIR", default_value = ".monowiki-collab")]
    pub workdir: PathBuf,

    /// Path to monowiki.yml relative to repo root
    #[arg(long, env = "MONOWIKI_CONFIG", default_value = "monowiki.yml")]
    pub config: PathBuf,

    /// Path to the monowiki CLI binary (defaults to `monowiki` in PATH)
    #[arg(long, env = "MONOWIKI_BIN")]
    pub monowiki_bin: Option<PathBuf>,

    /// Prefix for staging branches used during checkpointing
    #[arg(long, env = "MONOWIKI_STAGING_PREFIX", default_value = "collab/")]
    pub staging_prefix: String,

    /// Trigger a build right after startup (helpful for previews)
    #[arg(long)]
    pub build_on_start: bool,

    /// Enable debug logging
    #[arg(long, short)]
    pub verbose: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // Auth options
    // ─────────────────────────────────────────────────────────────────────────

    /// Secret for signing/verifying user JWT tokens (HS256).
    /// If not set, auth is disabled for user routes.
    #[arg(long, env = "MONOWIKI_USER_SECRET", hide_env_values = true)]
    pub user_secret: Option<String>,

    /// Secret for signing/verifying agent JWT tokens (HS256).
    /// Separate from user secret for easy revocation.
    #[arg(long, env = "MONOWIKI_AGENT_SECRET", hide_env_values = true)]
    pub agent_secret: Option<String>,

    /// Require authentication on all routes. If false, unauthenticated
    /// requests are allowed when no secret is configured.
    #[arg(long, env = "MONOWIKI_REQUIRE_AUTH", default_value = "false")]
    pub require_auth: bool,

    /// Expected JWT audience claim (optional).
    #[arg(long, env = "MONOWIKI_AUTH_AUDIENCE")]
    pub auth_audience: Option<String>,

    // ─────────────────────────────────────────────────────────────────────────
    // Rate limiting options
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable rate limiting for write operations.
    #[arg(long, env = "MONOWIKI_RATE_LIMIT", default_value = "true")]
    pub rate_limit: bool,

    /// Maximum burst size for rate limiting.
    #[arg(long, env = "MONOWIKI_RATE_BURST", default_value = "10")]
    pub rate_burst: u32,

    /// Sustained requests per second for rate limiting.
    #[arg(long, env = "MONOWIKI_RATE_PER_SEC", default_value = "1.0")]
    pub rate_per_sec: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Deploy options
    // ─────────────────────────────────────────────────────────────────────────

    /// Strategy for deploying to deploy branch: "subtree" or "split"
    #[arg(long, env = "MONOWIKI_DEPLOY_STRATEGY", default_value = "subtree")]
    pub deploy_strategy: String,
}
