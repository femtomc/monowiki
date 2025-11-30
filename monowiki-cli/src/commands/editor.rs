//! Editor server command - serves the collaborative editor UI.

use anyhow::Result;
use std::path::PathBuf;

/// Options for starting the editor stack (collab + embedded editor UI).
pub struct EditorStackOpts {
    pub repo: String,
    pub branch: String,
    pub listen_addr: String,
    pub workdir: PathBuf,
    pub config: PathBuf,
    pub build_on_start: bool,
    pub open_browser: bool,
    pub verbose: bool,
    pub in_place: bool,
}

/// Start the collab daemon with embedded editor UI (single origin).
pub async fn run_editor_stack(opts: EditorStackOpts) -> Result<()> {
    // monowiki-collab already serves the editor dist at "/" so we just start it.
    let collab_cli = monowiki_collab::cli::Cli {
        repo: opts.repo,
        branch: opts.branch,
        deploy_branch: None,
        listen_addr: opts.listen_addr.clone(),
        workdir: opts.workdir,
        config: opts.config,
        monowiki_bin: None,
        staging_prefix: "collab/".to_string(),
        build_on_start: opts.build_on_start,
        verbose: opts.verbose,
        user_secret: None,
        agent_secret: None,
        require_auth: false,
        auth_audience: None,
        rate_limit: false,
        rate_burst: 10,
        rate_per_sec: 1.0,
        deploy_strategy: "subtree".to_string(),
        in_place: opts.in_place,
    };

    let url = format!("http://{}", opts.listen_addr);
    tracing::info!("Starting monowiki editor at {}", url);
    println!("\nEditor:   {}", url);
    println!("Preview:  {}/preview", url);
    println!("Collab API: {}/api/status", url);
    println!("Press Ctrl+C to stop\n");

    if opts.open_browser {
        let _ = open::that(&url);
    }

    monowiki_collab::run_with_cli(collab_cli).await
}
