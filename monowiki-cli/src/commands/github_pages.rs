//! GitHub Pages setup command.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const WORKFLOW_TEMPLATE: &str = r#"name: Deploy to GitHub Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install monowiki
        run: |
          curl -sL https://github.com/femtomc/monowiki/releases/latest/download/monowiki-linux-x86_64.tar.gz | tar xz
          chmod +x monowiki
          sudo mv monowiki /usr/local/bin/

      - name: Update base_url for GitHub Pages
        run: |
          sed -i 's|base_url: "/"|base_url: "/{repo_name}/"|g' monowiki.yml
          cat monowiki.yml | grep base_url

      - name: Build documentation
        run: monowiki build

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: ./docs

  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    needs: build
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
"#;

/// Set up GitHub Actions workflow for GitHub Pages deployment
pub fn setup_github_pages(repo: Option<&str>, force: bool) -> Result<()> {
    let workflows_dir = Path::new(".github/workflows");
    let workflow_path = workflows_dir.join("deploy-pages.yml");

    // Check if workflow already exists
    if workflow_path.exists() && !force {
        anyhow::bail!(
            "GitHub Pages workflow already exists at {:?}\nUse --force to overwrite",
            workflow_path
        );
    }

    // Detect repository name from git remote if not provided
    let full_repo = if let Some(r) = repo {
        r.to_string()
    } else {
        detect_github_repo()?
    };

    // Extract just the repository name (without username)
    let repo_name = full_repo.split('/').nth(1).unwrap_or(&full_repo);

    // Create .github/workflows directory
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    // Write workflow file with repo name substituted
    let workflow_content = WORKFLOW_TEMPLATE.replace("{repo_name}", repo_name);
    fs::write(&workflow_path, workflow_content)
        .with_context(|| format!("Failed to write workflow to {:?}", workflow_path))?;

    println!(
        "✓ Created GitHub Actions workflow at {}",
        workflow_path.display()
    );
    println!();
    println!("Next steps:");
    println!("  1. Commit and push the workflow:");
    println!("     git add .github/workflows/deploy-pages.yml");
    println!("     git commit -m \"Add GitHub Pages deployment\"");
    println!("     git push");
    println!();
    println!("  2. Enable GitHub Pages in your repository settings:");
    println!(
        "     - Go to: https://github.com/{}/settings/pages",
        full_repo
    );
    println!("     - Source: GitHub Actions");
    println!();
    println!("  3. Push to main branch to trigger deployment");
    println!();
    println!(
        "Your site will be live at: https://{}.github.io/{}/",
        full_repo.split('/').next().unwrap_or(""),
        repo_name
    );

    Ok(())
}

fn detect_github_repo() -> Result<String> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("Failed to run git command - is this a git repository?")?;

    if !output.status.success() {
        anyhow::bail!("No git remote 'origin' found. Use --repo to specify repository name");
    }

    let url = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in git remote URL")?
        .trim()
        .to_string();

    // Parse repo name from various GitHub URL formats
    // https://github.com/user/repo.git → user/repo
    // git@github.com:user/repo.git → user/repo
    // https://github.com/user/repo → user/repo
    let clean_url = url.trim_end_matches(".git");

    // Try HTTPS format first (github.com/user/repo)
    if let Some(after_github) = clean_url.strip_prefix("https://github.com/") {
        return Ok(after_github.to_string());
    }

    // Try SSH format (git@github.com:user/repo)
    if let Some(after_colon) = clean_url.strip_prefix("git@github.com:") {
        return Ok(after_colon.to_string());
    }

    anyhow::bail!(
        "Could not parse GitHub repository from git remote URL: {}\nUse --repo user/repo to specify manually",
        url
    )
}
