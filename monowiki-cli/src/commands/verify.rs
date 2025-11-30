//! Verify vault health and emit diagnostics for agents.

use anyhow::{Context, Result};
use monowiki_core::{Config, SiteBuilder};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct VerificationSummary<'a> {
    notes: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
    diagnostics: &'a [monowiki_core::Diagnostic],
}

/// Run the build pipeline without rendering output and surface diagnostics.
pub fn verify_site(config_path: &Path, json: bool) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let builder = SiteBuilder::new(config.clone());
    let site_index = builder
        .build()
        .context("Failed to build site for verification")?;

    let diagnostics = site_index.diagnostics;
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == monowiki_core::DiagnosticSeverity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == monowiki_core::DiagnosticSeverity::Warning)
        .count();
    let infos = diagnostics
        .iter()
        .filter(|d| d.severity == monowiki_core::DiagnosticSeverity::Info)
        .count();

    let summary = VerificationSummary {
        notes: site_index.notes.len(),
        errors,
        warnings,
        infos,
        diagnostics: &diagnostics,
    };

    if json {
        let payload = serde_json::to_string_pretty(&summary)?;
        println!("{}", payload);
    } else {
        println!(
            "Verification complete: {} notes, {} errors, {} warnings, {} info",
            summary.notes, errors, warnings, infos
        );
        for diag in &diagnostics {
            let slug = diag
                .note_slug
                .as_deref()
                .map(|s| format!(" [{}]", s))
                .unwrap_or_default();
            let source = diag
                .source_path
                .as_deref()
                .map(|s| format!(" ({})", s))
                .unwrap_or_default();
            println!(
                "- {:?} {}{} {}: {}",
                diag.severity, diag.code, slug, source, diag.message
            );
            if let Some(ctx) = &diag.context {
                println!("  context: {}", ctx);
            }
        }
    }

    Ok(())
}
