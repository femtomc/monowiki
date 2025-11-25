use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tokio::process::Command;
use tracing::{info, warn, error, debug, instrument};

use crate::config::DeployStrategy;

/// Handles invoking `monowiki build` after git checkpoints.
#[derive(Debug, Clone)]
pub struct BuildRunner {
    monowiki_bin: PathBuf,
    worktree: PathBuf,
    config_path: PathBuf,
    deploy_branch: Option<String>,
    deploy_strategy: DeployStrategy,
}

impl BuildRunner {
    pub fn new(
        monowiki_bin: PathBuf,
        worktree: PathBuf,
        config_path: PathBuf,
        deploy_branch: Option<String>,
        deploy_strategy: DeployStrategy,
    ) -> Self {
        Self {
            monowiki_bin,
            worktree,
            config_path,
            deploy_branch,
            deploy_strategy,
        }
    }

    /// Placeholder for warming caches or checking the binary exists.
    pub async fn ensure_ready(&self) -> Result<()> {
        let status = Command::new(&self.monowiki_bin)
            .arg("--version")
            .output()
            .await?;
        if !status.status.success() {
            bail!(
                "monowiki binary {:?} not runnable (exit {:?})",
                self.monowiki_bin,
                status.status.code()
            );
        }
        info!(bin = %self.monowiki_bin.display(), "build runner ready");
        Ok(())
    }

    /// Kick off a build, optionally pushing to deploy branch.
    pub async fn run_build(&self) -> Result<()> {
        info!(bin = %self.monowiki_bin.display(), worktree = %self.worktree.display(), "running monowiki build");
        let status = Command::new(&self.monowiki_bin)
            .arg("build")
            .arg("--config")
            .arg(&self.config_path)
            .current_dir(&self.worktree)
            .output()
            .await?;

        if !status.status.success() {
            let stdout = String::from_utf8_lossy(&status.stdout);
            let stderr = String::from_utf8_lossy(&status.stderr);
            bail!(
                "monowiki build failed: exit {:?}\nstdout:\n{}\nstderr:\n{}",
                status.status.code(),
                stdout,
                stderr
            );
        }

        if let Some(branch) = &self.deploy_branch {
            self.push_to_deploy_branch(branch).await?;
        }

        Ok(())
    }

    /// Push the built output to the deploy branch.
    /// Supports two strategies:
    /// - Subtree: uses git subtree push (preserves history, slower)
    /// - Split: uses git subtree split + force push (faster, rewrites history)
    #[instrument(skip(self), fields(branch = %branch, strategy = ?self.deploy_strategy))]
    async fn push_to_deploy_branch(&self, branch: &str) -> Result<()> {
        // Read config to get output directory
        let config = monowiki_core::Config::from_file(&self.config_path)
            .context("failed to read config for deploy")?;
        let output_dir = config.output_dir();

        if !output_dir.exists() {
            error!(path = %output_dir.display(), "output directory does not exist after build");
            bail!("output directory {:?} does not exist after build", output_dir);
        }

        // Get relative path of output dir from worktree
        let output_rel = output_dir
            .strip_prefix(&self.worktree)
            .unwrap_or(&output_dir);
        let prefix = output_rel.to_string_lossy();

        info!(output = %output_dir.display(), prefix = %prefix, "preparing deploy");

        // First, make sure the output is committed to the current branch
        // (it might not be if it's in .gitignore)
        debug!("staging output directory");
        let add_output = Command::new("git")
            .args(["add", "-f", &prefix])
            .current_dir(&self.worktree)
            .output()
            .await?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            debug!(%stderr, "git add returned non-zero (may be ok if empty)");
        }

        // Check if there are changes to commit
        let status_output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.worktree)
            .output()
            .await?;

        let has_changes = !String::from_utf8_lossy(&status_output.stdout)
            .trim()
            .is_empty();

        if has_changes {
            debug!("committing build output");
            let commit = Command::new("git")
                .args(["commit", "-m", "Build output for deploy"])
                .current_dir(&self.worktree)
                .output()
                .await?;

            if !commit.status.success() {
                let stderr = String::from_utf8_lossy(&commit.stderr);
                warn!(%stderr, "git commit for build output failed (may be ok)");
            }
        }

        // Choose strategy
        match self.deploy_strategy {
            DeployStrategy::Subtree => {
                self.deploy_with_subtree_push(branch, &prefix).await
            }
            DeployStrategy::Split => {
                self.deploy_with_split_push(branch, &prefix).await
            }
        }
    }

    /// Deploy using git subtree push (preserves history)
    async fn deploy_with_subtree_push(&self, branch: &str, prefix: &str) -> Result<()> {
        info!(branch, prefix, "deploying with subtree push");

        let subtree_push = Command::new("git")
            .args(["subtree", "push", "--prefix", prefix, "origin", branch])
            .current_dir(&self.worktree)
            .output()
            .await?;

        if subtree_push.status.success() {
            info!(branch, "subtree push succeeded");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&subtree_push.stderr);
        warn!(%stderr, "subtree push failed, falling back to split approach");

        // Fall back to split approach
        self.deploy_with_split_push(branch, prefix).await
    }

    /// Deploy using git subtree split + force push (faster, rewrites history)
    async fn deploy_with_split_push(&self, branch: &str, prefix: &str) -> Result<()> {
        info!(branch, prefix, "deploying with split + force push");

        // Create a subtree split
        let split = Command::new("git")
            .args(["subtree", "split", "--prefix", prefix, "-b", "_deploy_temp"])
            .current_dir(&self.worktree)
            .output()
            .await?;

        if !split.status.success() {
            let stderr = String::from_utf8_lossy(&split.stderr);
            error!(%stderr, "git subtree split failed");
            bail!("git subtree split failed: {}", stderr);
        }

        debug!("subtree split created _deploy_temp branch");

        // Force push to deploy branch
        let push = Command::new("git")
            .args(["push", "origin", &format!("_deploy_temp:{}", branch), "--force"])
            .current_dir(&self.worktree)
            .output()
            .await?;

        // Clean up temp branch regardless of push result
        let _ = Command::new("git")
            .args(["branch", "-D", "_deploy_temp"])
            .current_dir(&self.worktree)
            .output()
            .await;

        if !push.status.success() {
            let stderr = String::from_utf8_lossy(&push.stderr);
            error!(%stderr, "git push to deploy branch failed");
            bail!("git push to deploy branch failed: {}", stderr);
        }

        info!(branch, "successfully pushed to deploy branch");
        Ok(())
    }

    pub fn summary(&self) -> BuildSummary {
        BuildSummary {
            monowiki_bin: self.monowiki_bin.clone(),
            worktree: self.worktree.clone(),
            config_path: self.config_path.clone(),
            deploy_branch: self.deploy_branch.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildSummary {
    pub monowiki_bin: PathBuf,
    pub worktree: PathBuf,
    pub config_path: PathBuf,
    pub deploy_branch: Option<String>,
}
