//! Configuration parsing and management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse YAML: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Main configuration struct matching monowiki.yml schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    pub paths: PathsConfig,

    #[serde(default)]
    pub orcid: Option<OrcidConfig>,

    #[serde(default)]
    pub server: ServerConfig,

    // NEW FIELDS for Rust version
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    #[serde(default)]
    pub bibliography: Vec<PathBuf>,

    #[serde(default)]
    pub theme_overrides: Option<PathBuf>,

    #[serde(default = "default_true")]
    pub enable_rss: bool,

    #[serde(default = "default_true")]
    pub enable_sitemap: bool,

    #[serde(default = "default_true")]
    pub enable_backlinks: bool,

    #[serde(default)]
    pub adapters: Vec<AdapterConfig>,

    // Internal: path to config file (for relative path resolution)
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

fn default_base_url() -> String {
    String::from("/")
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub author: String,
    pub description: String,
    pub url: String,

    #[serde(default)]
    pub intro: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub vault: PathBuf,
    pub output: PathBuf,

    #[serde(default)]
    pub templates: Option<PathBuf>,

    #[serde(default)]
    pub theme: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrcidConfig {
    pub id: String,

    #[serde(default = "default_cache_hours")]
    pub cache_hours: u64,
}

fn default_cache_hours() -> u64 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub name: String,
    pub source_path: PathBuf,
    pub output_dir: PathBuf,

    #[serde(default)]
    pub repo_url: Option<String>,

    #[serde(default)]
    pub options: HashMap<String, serde_yaml::Value>,
}

impl Config {
    /// Load configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&contents)?;

        // Store config file path for relative path resolution
        config.config_path = Some(path.to_path_buf());

        Ok(config)
    }

    /// Get the vault directory, resolved relative to config file
    pub fn vault_dir(&self) -> PathBuf {
        self.resolve_path(&self.paths.vault)
    }

    /// Get the output directory, resolved relative to config file
    pub fn output_dir(&self) -> PathBuf {
        self.resolve_path(&self.paths.output)
    }

    /// Resolve an arbitrary path relative to the config file location
    pub fn resolve_relative(&self, path: &Path) -> PathBuf {
        self.resolve_path(path)
    }

    /// Get bibliography files, resolved relative to config file
    pub fn bibliography_paths(&self) -> Vec<PathBuf> {
        self.bibliography
            .iter()
            .map(|p| self.resolve_path(p))
            .collect()
    }

    /// Get the templates directory (None means use built-in)
    pub fn templates_dir(&self) -> Option<PathBuf> {
        self.paths.templates.as_ref().map(|p| self.resolve_path(p))
    }

    /// Get the theme directory (None means use built-in)
    pub fn theme_dir(&self) -> Option<PathBuf> {
        self.paths.theme.as_ref().map(|p| self.resolve_path(p))
    }

    /// Get theme overrides directory (copied after the main theme)
    pub fn theme_overrides_dir(&self) -> Option<PathBuf> {
        self.theme_overrides.as_ref().map(|p| self.resolve_path(p))
    }

    /// Resolve a path relative to the config file location
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(config_path) = &self.config_path {
            if let Some(parent) = config_path.parent() {
                parent.join(path)
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        }
    }

    /// Get a nested config value using dotted path (e.g., "site.title")
    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["site", "title"] => Some(self.site.title.clone()),
            ["site", "author"] => Some(self.site.author.clone()),
            ["site", "description"] => Some(self.site.description.clone()),
            ["site", "url"] => Some(self.site.url.clone()),
            ["site", "intro"] => self.site.intro.clone(),
            ["server", "port"] => Some(self.server.port.to_string()),
            _ => None,
        }
    }

    /// Normalized base URL with leading and trailing slash ("/foo/" or "/")
    pub fn normalized_base_url(&self) -> String {
        normalize_base_url(&self.base_url)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
        }
    }
}

/// Ensure base URLs have a leading and trailing slash
pub fn normalize_base_url(raw: &str) -> String {
    if raw.is_empty() {
        return "/".to_string();
    }

    let mut s = raw.trim().to_string();
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    if !s.ends_with('/') {
        s.push('/');
    }

    // Collapse duplicate slashes (but keep leading)
    while s.contains("//") {
        s = s.replace("//", "/");
        if !s.starts_with('/') {
            s.insert(0, '/');
        }
    }

    if s.is_empty() {
        "/".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = Config {
            site: SiteConfig {
                title: "Test".into(),
                author: "Author".into(),
                description: "Desc".into(),
                url: "https://example.com".into(),
                intro: None,
            },
            paths: PathsConfig {
                vault: PathBuf::from("vault"),
                output: PathBuf::from("docs"),
                templates: None,
                theme: None,
            },
            orcid: None,
            server: ServerConfig::default(),
            base_url: default_base_url(),
            ignore_patterns: vec![],
            bibliography: vec![],
            theme_overrides: None,
            enable_rss: true,
            enable_sitemap: true,
            enable_backlinks: true,
            adapters: vec![],
            config_path: None,
        };

        assert_eq!(config.base_url, "/");
        assert_eq!(config.server.port, 8000);
        assert!(config.enable_rss);
    }

    #[test]
    fn test_get_nested_value() {
        let config = Config {
            site: SiteConfig {
                title: "My Site".into(),
                author: "John Doe".into(),
                description: "A test site".into(),
                url: "https://example.com".into(),
                intro: Some("Welcome!".into()),
            },
            paths: PathsConfig {
                vault: PathBuf::from("vault"),
                output: PathBuf::from("docs"),
                templates: None,
                theme: None,
            },
            orcid: None,
            server: ServerConfig::default(),
            base_url: default_base_url(),
            ignore_patterns: vec![],
            bibliography: vec![],
            theme_overrides: None,
            enable_rss: true,
            enable_sitemap: true,
            enable_backlinks: true,
            adapters: vec![],
            config_path: None,
        };

        assert_eq!(config.get("site.title"), Some("My Site".into()));
        assert_eq!(config.get("site.author"), Some("John Doe".into()));
        assert_eq!(config.get("site.intro"), Some("Welcome!".into()));
        assert_eq!(config.get("server.port"), Some("8000".into()));
        assert_eq!(config.get("nonexistent.key"), None);
    }
}
