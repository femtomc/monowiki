//! monowiki-collab: realtime editor + git checkpoint scaffolding.
//!
//! This is a thin daemon that will host WebSocket collab, checkpoint to git,
//! and trigger `monowiki build`. Right now it only wires config + HTTP stubs so
//! we can iterate incrementally without touching the existing build pipeline.

mod auth;
mod build;
mod cli;
mod config;
mod git;
mod ratelimit;
mod server;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::{build::BuildRunner, config::CollabConfig, git::GitWorkspace};

fn init_tracing(verbose: bool) -> Result<()> {
    let level = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    init_tracing(cli.verbose)?;

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
