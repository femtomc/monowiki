use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use tracing::info;
use tokio::process::Command;

/// Manages the git worktree used for checkpointing CRDT state.
#[derive(Debug, Clone)]
pub struct GitWorkspace {
    repo: String,
    branch: String,
    deploy_branch: Option<String>,
    workdir: PathBuf,
    staging_prefix: Option<String>,
}

impl GitWorkspace {
    pub fn new(
        repo: String,
        branch: String,
        deploy_branch: Option<String>,
        workdir: PathBuf,
        staging_prefix: Option<String>,
    ) -> Self {
        Self {
            repo,
            branch,
            deploy_branch,
            workdir,
            staging_prefix,
        }
    }

    /// Create directories for the worktree/cache. Actual git clone/pull happens later.
    pub async fn prepare(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.workdir)
            .await
            .with_context(|| format!("create workdir at {:?}", self.workdir))?;

        info!(
            repo = %self.repo,
            branch = %self.branch,
            workdir = %self.workdir.display(),
            "git workspace prepared"
        );

        Ok(())
    }

    /// Ensure we have a checkout of the target branch. If not cloned, clone; otherwise fetch/rebase.
    pub async fn init_or_refresh(&self) -> Result<()> {
        if !self.worktree_path().join(".git").exists() {
            self.clone_branch().await?;
        } else {
            self.pull_rebase().await?;
        }
        Ok(())
    }

    /// Fetch + rebase the target branch.
    pub async fn pull_rebase(&self) -> Result<()> {
        self.git(["fetch", "--all", "--prune"]).await?;
        self.git(["checkout", &self.branch]).await?;
        self.git(["pull", "--rebase", "origin", &self.branch]).await?;
        self.git([
            "branch",
            "--set-upstream-to",
            &format!("origin/{}", self.branch),
            &self.branch,
        ])
        .await
        .ok(); // best-effort
        Ok(())
    }

    /// Stage everything under vault for commit.
    pub async fn add_vault(&self) -> Result<()> {
        self.git(["add", "vault"]).await
    }

    /// Whether there are staged or unstaged changes.
    pub async fn has_changes(&self) -> Result<bool> {
        let out = self.git_output(["status", "--porcelain"]).await?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Commit pending changes. Returns whether a commit was created.
    pub async fn commit(&self, message: &str, allow_empty: bool) -> Result<bool> {
        if !allow_empty && !self.has_changes().await? {
            return Ok(false);
        }

        let mut args = vec!["commit", "-m", message];
        if allow_empty {
            args.push("--allow-empty");
        }
        self.git(args).await?;
        Ok(true)
    }

    /// Push the current branch.
    pub async fn push(&self) -> Result<()> {
        self.git(["push", "origin", &self.branch]).await
    }

    /// Convenience for staging branch name.
    pub fn staging_branch_for(&self, note_id: &str) -> Option<String> {
        self.staging_prefix
            .as_ref()
            .map(|prefix| format!("{}{}", prefix, note_id))
    }

    pub fn worktree_path(&self) -> PathBuf {
        self.workdir.join("worktree")
    }

    pub fn repo_summary(&self) -> GitWorkspaceSummary {
        GitWorkspaceSummary {
            repo: self.repo.clone(),
            branch: self.branch.clone(),
            deploy_branch: self.deploy_branch.clone(),
            workdir: self.workdir.clone(),
            staging_prefix: self.staging_prefix.clone(),
        }
    }

    async fn clone_branch(&self) -> Result<()> {
        info!(
            repo = %self.repo,
            branch = %self.branch,
            dest = %self.worktree_path().display(),
            "cloning repository"
        );
        tokio::fs::create_dir_all(&self.workdir)
            .await
            .with_context(|| format!("create workdir {:?}", self.workdir))?;
        // Clone using the parent of the target dir as cwd so git can create the dest folder.
        let parent = self
            .worktree_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.workdir.clone());
        let mut args = vec![
            "clone".to_string(),
            "--branch".to_string(),
            self.branch.clone(),
            self.repo.clone(),
            self.worktree_path_str(),
        ];
        let out = self.git_in_dir(args.iter().map(|s| s.as_str()), &parent).await?;
        if !out.status.success() {
            return Err(anyhow!(
                "git clone failed with exit {:?}",
                out.status.code()
            ));
        }
        Ok(())
    }

    async fn git<I, S>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let output = self.git_output(args).await?;
        if !output.status.success() {
            return Err(anyhow!(
                "git exited with {}",
                output.status.code().unwrap_or(-1)
            ));
        }
        Ok(())
    }

    async fn git_output<I, S>(&self, args: I) -> Result<std::process::Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.git_in_dir(args, &self.worktree_path()).await
    }

    fn worktree_path_str(&self) -> String {
        self.worktree_path()
            .to_str()
            .unwrap_or_else(|| panic!("non-utf8 worktree path {:?}", self.worktree_path()))
            .to_string()
    }

    async fn git_in_dir<I, S>(&self, args: I, cwd: &Path) -> Result<std::process::Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut cmd = Command::new("git");
        cmd.current_dir(cwd);
        for arg in args {
            cmd.arg(arg.as_ref());
        }
        let output = cmd
            .output()
            .await
            .with_context(|| format!("failed to run git {:?}", cmd))?;
        Ok(output)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GitWorkspaceSummary {
    pub repo: String,
    pub branch: String,
    pub deploy_branch: Option<String>,
    pub workdir: PathBuf,
    pub staging_prefix: Option<String>,
}
