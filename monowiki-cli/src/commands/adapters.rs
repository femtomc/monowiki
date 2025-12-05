//! Run code documentation adapters and emit markdown into the vault.

use anyhow::{Context, Result};
use monowiki_adapters::{adapter_by_name, AdapterOptions};
use monowiki_core::Config;
use std::fs;

/// Execute all configured adapters, writing generated markdown into their output directories.
pub fn run_doc_adapters(config: &Config) -> Result<()> {
    if config.adapters.is_empty() {
        return Ok(());
    }

    for adapter_cfg in &config.adapters {
        let Some(adapter) = adapter_by_name(&adapter_cfg.name) else {
            tracing::warn!("Unknown adapter '{}'; skipping", adapter_cfg.name);
            continue;
        };

        let source_root = config.resolve_relative(&adapter_cfg.source_path);
        let output_dir = config.resolve_relative(&adapter_cfg.output_dir);
        let options = AdapterOptions::from_map(adapter_cfg.options.clone());

        tracing::info!(
            adapter = %adapter_cfg.name,
            source = %source_root.display(),
            output = %output_dir.display(),
            "Running documentation adapter"
        );

        fs::create_dir_all(&output_dir)
            .with_context(|| format!("Failed to create adapter output dir {:?}", output_dir))?;

        let rendered = adapter.extract(&source_root, adapter_cfg.repo_url.as_deref(), &options)?;
        tracing::info!(adapter = %adapter_cfg.name, count = rendered.len(), "Adapter produced documents");

        for output in &rendered {
            let dest = output_dir.join(&output.output_rel_path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {:?}", parent))?;
            }
            let markdown = output.to_markdown()?;
            let should_write = match fs::read_to_string(&dest) {
                Ok(existing) => existing != markdown,
                Err(_) => true,
            };
            if should_write {
                fs::write(&dest, markdown)
                    .with_context(|| format!("Failed to write generated doc {}", dest.display()))?;
            }
        }

        tracing::info!(adapter = %adapter_cfg.name, written = rendered.len(), "Documentation files on disk");

        // Spot check a module doc if present in outputs.
        if rendered
            .iter()
            .any(|o| o.output_rel_path.ends_with("slug/module.md"))
        {
            let check = output_dir.join("slug/module.md");
            tracing::info!(exists = check.exists(), path = %check.display(), "Module doc spot-check");
        }
    }

    Ok(())
}
