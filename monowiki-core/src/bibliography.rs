//! Bibliography loading and lookup for citation support.

use hayagriva::{io::from_biblatex_str, Entry, Library};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use tracing::warn;

/// Cached bibliography loader to avoid re-reading the same `.bib` files.
#[derive(Debug, Default)]
pub struct BibliographyStore {
    cache: HashMap<PathBuf, Library>,
}

impl BibliographyStore {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Ensure the given paths are loaded into the cache.
    pub fn preload_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.ensure_loaded(path);
        }
    }

    /// Build a merged bibliography for the provided list of paths.
    ///
    /// Later files win on key conflicts.
    pub fn collect(&mut self, paths: &[PathBuf]) -> Bibliography {
        self.preload_paths(paths);

        let mut entries: HashMap<String, Entry> = HashMap::new();
        for path in paths {
            if let Some(lib) = self.cache.get(path) {
                for entry in lib.iter() {
                    entries.insert(entry.key().to_string(), entry.clone());
                }
            }
        }

        Bibliography { entries }
    }

    fn ensure_loaded(&mut self, path: &Path) {
        if self.cache.contains_key(path) {
            return;
        }

        match fs::read_to_string(path) {
            Ok(contents) => match from_biblatex_str(&contents) {
                Ok(lib) => {
                    self.cache.insert(path.to_path_buf(), lib);
                }
                Err(errors) => {
                    let joined = errors
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("; ");
                    warn!("Failed to parse bibliography {:?}: {}", path, joined);
                    self.cache.insert(path.to_path_buf(), Library::new());
                }
            },
            Err(err) => {
                warn!("Failed to read bibliography {:?}: {}", path, err);
                self.cache.insert(path.to_path_buf(), Library::new());
            }
        }
    }
}

/// Resolved bibliography entries for a single note.
#[derive(Debug, Clone, Default)]
pub struct Bibliography {
    entries: HashMap<String, Entry>,
}

impl Bibliography {
    /// Lookup a bibliography entry by key.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Returns true if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
