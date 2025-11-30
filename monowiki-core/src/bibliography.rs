//! Bibliography loading and lookup for citation support.

use hayagriva::{io::from_biblatex_str, Entry, Library};
use std::{
    collections::HashMap,
    fs, mem,
    path::{Path, PathBuf},
};
use tracing::warn;

use crate::models::{Diagnostic, DiagnosticSeverity};

/// Cached bibliography loader to avoid re-reading the same `.bib` files.
#[derive(Debug, Default)]
pub struct BibliographyStore {
    cache: HashMap<PathBuf, Library>,
    diagnostics: Vec<Diagnostic>,
}

impl BibliographyStore {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            diagnostics: Vec::new(),
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
                    self.diagnostics.push(Diagnostic {
                        code: "bibliography.load_failed".to_string(),
                        message: format!("Failed to parse bibliography: {}", joined),
                        severity: DiagnosticSeverity::Warning,
                        note_slug: None,
                        source_path: Some(path.to_string_lossy().to_string()),
                        context: None,
                        anchor: None,
                    });
                    self.cache.insert(path.to_path_buf(), Library::new());
                }
            },
            Err(err) => {
                warn!("Failed to read bibliography {:?}: {}", path, err);
                self.diagnostics.push(Diagnostic {
                    code: "bibliography.load_failed".to_string(),
                    message: format!("Failed to read bibliography: {}", err),
                    severity: DiagnosticSeverity::Warning,
                    note_slug: None,
                    source_path: Some(path.to_string_lossy().to_string()),
                    context: None,
                    anchor: None,
                });
                self.cache.insert(path.to_path_buf(), Library::new());
            }
        }
    }

    /// Take accumulated diagnostics (clearing the internal buffer).
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        mem::take(&mut self.diagnostics)
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
