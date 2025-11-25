use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::auth::AuthConfig;
use crate::cli::Cli;
use crate::ratelimit::RateLimitConfig;

/// Deploy strategy for pushing to deploy branch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployStrategy {
    /// Use git subtree push (default, non-destructive)
    Subtree,
    /// Use split + force push (faster but rewrites history)
    Split,
}

impl DeployStrategy {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "split" => Self::Split,
            _ => Self::Subtree,
        }
    }
}

/// Runtime configuration derived from CLI/env.
#[derive(Debug, Clone)]
pub struct CollabConfig {
    pub repo: String,
    pub branch: String,
    pub deploy_branch: Option<String>,
    pub deploy_strategy: DeployStrategy,
    pub listen_addr: String,
    pub workdir: PathBuf,
    pub config_rel: PathBuf,
    pub monowiki_bin: Option<PathBuf>,
    pub staging_prefix: String,
    pub build_on_start: bool,
    pub auth: AuthConfig,
    pub rate_limit: RateLimitConfig,
}

impl CollabConfig {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let workdir = if cli.workdir.is_relative() {
            std::env::current_dir()?.join(&cli.workdir)
        } else {
            cli.workdir.clone()
        };
        // Store config path relative to the repo root (worktree). Absolute paths are kept as-is.
        let config_rel = cli.config.clone();

        let auth = AuthConfig {
            user_secret: cli.user_secret.clone(),
            agent_secret: cli.agent_secret.clone(),
            expected_aud: cli.auth_audience.clone(),
            require_auth: cli.require_auth,
        };

        let rate_limit = RateLimitConfig {
            burst: cli.rate_burst,
            refill_rate: cli.rate_per_sec,
            enabled: cli.rate_limit,
        };

        Ok(Self {
            repo: cli.repo.clone(),
            branch: cli.branch.clone(),
            deploy_branch: cli.deploy_branch.clone(),
            deploy_strategy: DeployStrategy::from_str(&cli.deploy_strategy),
            listen_addr: cli.listen_addr.clone(),
            workdir,
            config_rel,
            monowiki_bin: cli.monowiki_bin.clone(),
            staging_prefix: cli.staging_prefix.clone(),
            build_on_start: cli.build_on_start,
            auth,
            rate_limit,
        })
    }

    pub fn worktree_path(&self) -> PathBuf {
        self.workdir.join("worktree")
    }

    pub fn cache_path(&self) -> PathBuf {
        self.workdir.join("cache")
    }

    pub fn vault_path(&self) -> PathBuf {
        self.worktree_path().join("vault")
    }

    /// Path to monowiki.yml inside the worktree.
    pub fn config_path(&self) -> PathBuf {
        if self.config_rel.is_absolute() {
            self.config_rel.clone()
        } else {
            self.worktree_path().join(&self.config_rel)
        }
    }

    pub fn monowiki_bin(&self) -> &Path {
        self.monowiki_bin
            .as_deref()
            .unwrap_or_else(|| Path::new("monowiki"))
    }
}
