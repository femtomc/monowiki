//! Disk cache for site indexes to avoid rebuilding in read-only CLI commands.

use anyhow::{Context, Result};
use chrono::Utc;
use monowiki_core::{Config, SiteBuilder, SiteIndex};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_VERSION: &str = "1";
const CACHE_FILENAME: &str = ".site_index.json";

#[derive(Serialize, Deserialize)]
struct CachedSiteIndex {
    version: String,
    generated_at: String,
    site_index: SiteIndex,
}

fn cache_path(config: &Config) -> PathBuf {
    config.output_dir().join(CACHE_FILENAME)
}

/// Persist the full site index next to build artifacts.
pub fn write_site_index_cache(config: &Config, site_index: &SiteIndex) -> Result<()> {
    let path = cache_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache dir {:?}", parent))?;
    }

    let payload = CachedSiteIndex {
        version: CACHE_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        site_index: site_index.clone(),
    };

    let json = serde_json::to_vec(&payload).context("Failed to serialize site index cache")?;
    fs::write(&path, json).with_context(|| format!("Failed to write cache {:?}", path))?;
    Ok(())
}

/// Load the cached site index if present and compatible.
pub fn load_cached_site_index(config: &Config) -> Result<Option<SiteIndex>> {
    let path = cache_path(config);
    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read(&path).with_context(|| format!("Failed to read cache {:?}", path))?;
    match serde_json::from_slice::<CachedSiteIndex>(&data) {
        Ok(cache) if cache.version == CACHE_VERSION => Ok(Some(cache.site_index)),
        Ok(_) => Ok(None),
        Err(err) => {
            tracing::warn!("Failed to parse site index cache: {}", err);
            Ok(None)
        }
    }
}

/// Load config and site index, preferring the cache but falling back to a rebuild.
pub fn load_or_build_site_index(config_path: &Path) -> Result<(Config, SiteIndex)> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;

    if let Some(index) = load_cached_site_index(&config)? {
        return Ok((config, index));
    }

    let builder = SiteBuilder::new(config.clone());
    let site_index = builder.build().context("Failed to build site index")?;

    if let Err(err) = write_site_index_cache(&config, &site_index) {
        tracing::warn!("Could not write site index cache: {}", err);
    }

    Ok((config, site_index))
}
