//! Library entrypoint for monowiki-collab so other binaries (monowiki CLI) can
//! reuse the server without shelling out.

pub mod auth;
pub mod build;
pub mod cli;
pub mod config;
pub mod crdt;
pub mod git;
pub mod ratelimit;
pub mod render;
pub mod server;

// New CRDT abstraction layer (Sprint 03)
pub mod operational;
pub mod yrs_adapter;
pub mod projection;
pub mod migration;

#[cfg(feature = "loro")]
pub mod loro;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use crate::{build::BuildRunner, config::CollabConfig, git::GitWorkspace};

fn try_init_tracing(verbose: bool) {
    let level = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    // Use try_init to avoid panic if tracing is already set up (e.g., by CLI)
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Run the collab daemon using CLI args (parsed by the caller).
pub async fn run_with_cli(cli: cli::Cli) -> Result<()> {
    try_init_tracing(cli.verbose);

    let cfg = CollabConfig::from_cli(&cli)?;
    let workspace = GitWorkspace::new(
        cfg.repo.clone(),
        cfg.branch.clone(),
        cfg.deploy_branch.clone(),
        cfg.workdir.clone(),
        Some(cfg.staging_prefix.clone()),
    );
    workspace.prepare().await?;

    let builder = BuildRunner::new(
        cfg.monowiki_bin().to_path_buf(),
        cfg.worktree_path(),
        cfg.config_path(),
        cfg.deploy_branch.clone(),
        cfg.deploy_strategy,
    );

    server::serve(cfg, workspace, builder).await
}
