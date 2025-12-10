//! Python adapter using tree-sitter for parsing.
//!
//! Extracts documentation from Python source files including:
//! - Functions and methods
//! - Classes
//! - Module-level constants

use crate::{
    build_body, build_frontmatter, build_output_path, is_test_path, walk_source_files,
    AdapterError, AdapterOptions, AdapterOutput, DocAdapter, DocItem, DocKind, SourceLocation,
};
use std::path::Path;
use tracing::{debug, warn};
use tree_sitter::{Node, Parser, Tree};

/// Python documentation adapter using tree-sitter
pub struct PythonAdapter {
    parser: std::sync::Mutex<Parser>,
}

impl PythonAdapter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("Failed to set Python language for parser");
        Self {
            parser: std::sync::Mutex::new(parser),
        }
    }

    fn parse(&self, source: &str) -> Result<Tree, AdapterError> {
        let mut parser = self.parser.lock().map_err(|e| {
            AdapterError::ParseError(format!("Failed to acquire parser lock: {}", e))
        })?;
        parser
            .parse(source, None)
            .ok_or_else(|| AdapterError::ParseError("Failed to parse source".to_string()))
    }

    fn extract_from_file(
        &self,
        path: &Path,
        source_root: &Path,
        repo_url: Option<&str>,
        options: &AdapterOptions,
    ) -> Result<Vec<DocItem>, AdapterError> {
        let source = std::fs::read_to_string(path)?;
        let tree = self.parse(&source)?;
        let source_bytes = source.as_bytes();

        let rel_path = path
            .strip_prefix(source_root)
            .unwrap_or(path)
            .to_path_buf();

        let module_path = derive_module_path(&rel_path);
        let include_private = options.get_bool("include_private", false);

        let mut items = Vec::new();
        let root = tree.root_node();

        self.walk_module(
            root,
            source_bytes,
            &rel_path,
            repo_url,
            &module_path,
            include_private,
            None,
            &mut items,
        );

        debug!(
            "Extracted {} items from {}",
            items.len(),
            path.display()
        );

        Ok(items)
    }

    fn walk_module(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
        class_name: Option<&str>,
        items: &mut Vec<DocItem>,
    ) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(item) = self.extract_function(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        class_name,
                        include_private,
                    ) {
                        items.push(item);
                    }
                }
                "class_definition" => {
                    if let Some((class_item, class_name_str)) = self.extract_class(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                    ) {
                        items.push(class_item);

                        // Extract methods from the class body
                        if let Some(body) = child.child_by_field_name("body") {
                            self.walk_module(
                                body,
                                source,
                                rel_path,
                                repo_url,
                                module_path,
                                include_private,
                                Some(&class_name_str),
                                items,
                            );
                        }
                    }
                }
                "expression_statement" => {
                    // Could be a module-level assignment (constant)
                    // Skip for now - focus on functions and classes
                }
                _ => {}
            }
        }
    }

    fn extract_function(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        class_name: Option<&str>,
        include_private: bool,
    ) -> Option<DocItem> {
        let name_node = node.child_by_field_name("name")?;
        let func_name = node_text(name_node, source)?;

        // Skip private functions unless requested
        if func_name.starts_with('_') && !func_name.starts_with("__") && !include_private {
            return None;
        }

        // Skip test functions
        if func_name.starts_with("test_") {
            return None;
        }

        // Skip dunder methods except __init__
        if func_name.starts_with("__") && func_name.ends_with("__") && func_name != "__init__" {
            return None;
        }

        let docs = extract_docstring(node, source);
        let full_name = build_full_name(module_path, class_name, &func_name);
        let signature = extract_function_signature(node, source)?;
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: if class_name.is_some() {
                DocKind::Method
            } else {
                DocKind::Function
            },
            docs,
            signature,
            source: Some(item_source),
            location: SourceLocation {
                file: rel_path.to_path_buf(),
                start_line: Some(node.start_position().row as u32 + 1),
                end_line: Some(node.end_position().row as u32 + 1),
                repo_url: repo_url.map(String::from),
            },
            module_path: module_path.to_vec(),
            container: class_name.map(String::from),
        })
    }

    fn extract_class(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<(DocItem, String)> {
        let name_node = node.child_by_field_name("name")?;
        let class_name = node_text(name_node, source)?;

        // Skip private classes unless requested
        if class_name.starts_with('_') && !include_private {
            return None;
        }

        let docs = extract_docstring(node, source);
        let full_name = build_full_name(module_path, None, &class_name);
        let signature = extract_class_signature(node, source)?;
        let item_source = node_text(node, source)?;

        let item = DocItem {
            name: full_name,
            kind: DocKind::Class,
            docs,
            signature,
            source: Some(item_source),
            location: SourceLocation {
                file: rel_path.to_path_buf(),
                start_line: Some(node.start_position().row as u32 + 1),
                end_line: Some(node.end_position().row as u32 + 1),
                repo_url: repo_url.map(String::from),
            },
            module_path: module_path.to_vec(),
            container: None,
        };

        Some((item, class_name))
    }
}

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DocAdapter for PythonAdapter {
    fn name(&self) -> &str {
        "python"
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract(
        &self,
        source_root: &Path,
        repo_url: Option<&str>,
        options: &AdapterOptions,
    ) -> Result<Vec<AdapterOutput>, AdapterError> {
        let skip_tests = options.get_bool("skip_tests", true);
        let mut outputs = Vec::new();

        for path in walk_source_files(source_root, self.extensions()) {
            // Skip test files if configured
            if skip_tests && is_test_path(&path) {
                debug!("Skipping test file: {}", path.display());
                continue;
            }

            // Skip __pycache__ and other common non-source dirs
            if path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.starts_with("__pycache__") || s.starts_with(".") || s == "venv" || s == "env"
            }) {
                continue;
            }

            match self.extract_from_file(&path, source_root, repo_url, options) {
                Ok(items) => {
                    for item in items {
                        let frontmatter = build_frontmatter(&item, "python");
                        let body_md = build_body(&item, "python");
                        let output_rel_path = build_output_path(&item);

                        outputs.push(AdapterOutput {
                            output_rel_path,
                            frontmatter,
                            body_md,
                            source: item.location.clone(),
                            language: "python".to_string(),
                            kind: item.kind,
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to extract from {}: {}", path.display(), e);
                }
            }
        }

        Ok(outputs)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn node_text(node: Node, source: &[u8]) -> Option<String> {
    std::str::from_utf8(&source[node.byte_range()])
        .ok()
        .map(|s| s.to_string())
}

fn derive_module_path(rel_path: &Path) -> Vec<String> {
    let mut parts: Vec<String> = rel_path
        .with_extension("")
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(String::from))
        .collect();

    // Remove __init__ if it's the last component
    if parts.last().map(|s| s.as_str()) == Some("__init__") {
        parts.pop();
    }

    parts
}

fn build_full_name(module_path: &[String], class_name: Option<&str>, name: &str) -> String {
    let mut parts: Vec<&str> = module_path.iter().map(|s| s.as_str()).collect();
    if let Some(c) = class_name {
        parts.push(c);
    }
    parts.push(name);
    parts.join(".")
}

/// Extract docstring from a function or class definition
fn extract_docstring(node: Node, source: &[u8]) -> Option<String> {
    // In Python, docstrings are the first string in the body
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "string" {
                    let text = node_text(inner, source)?;
                    // Remove quotes and clean up
                    let cleaned = text
                        .trim_start_matches("\"\"\"")
                        .trim_start_matches("'''")
                        .trim_start_matches('"')
                        .trim_start_matches('\'')
                        .trim_end_matches("\"\"\"")
                        .trim_end_matches("'''")
                        .trim_end_matches('"')
                        .trim_end_matches('\'')
                        .trim();
                    if !cleaned.is_empty() {
                        return Some(cleaned.to_string());
                    }
                }
            }
        }
        // Only check the first statement
        break;
    }

    None
}

fn extract_function_signature(node: Node, source: &[u8]) -> Option<String> {
    // Build signature from def name(params) -> return_type:
    let mut sig = String::from("def ");

    let name_node = node.child_by_field_name("name")?;
    sig.push_str(&node_text(name_node, source)?);

    let params_node = node.child_by_field_name("parameters")?;
    sig.push_str(&node_text(params_node, source)?);

    // Check for return type annotation
    if let Some(return_type) = node.child_by_field_name("return_type") {
        sig.push_str(" -> ");
        sig.push_str(&node_text(return_type, source)?);
    }

    Some(sig)
}

fn extract_class_signature(node: Node, source: &[u8]) -> Option<String> {
    let mut sig = String::from("class ");

    let name_node = node.child_by_field_name("name")?;
    sig.push_str(&node_text(name_node, source)?);

    // Check for base classes
    if let Some(bases) = node.child_by_field_name("superclasses") {
        sig.push_str(&node_text(bases, source)?);
    }

    Some(sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_adapter() -> PythonAdapter {
        PythonAdapter::new()
    }

    #[test]
    fn test_parse_simple_function() {
        let adapter = make_adapter();
        let source = r#"
def hello(name: str) -> str:
    """Greet someone."""
    return f"Hello, {name}!"
"#;
        let tree = adapter.parse(source).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "module");
    }

    #[test]
    fn test_extract_function() {
        let adapter = make_adapter();
        let source = r#"
def add(a: int, b: int) -> int:
    """Add two numbers together."""
    return a + b
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("math.py");

        let mut items = Vec::new();
        adapter.walk_module(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["math".to_string()],
            false,
            None,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "math.add");
        assert_eq!(item.kind, DocKind::Function);
        assert_eq!(item.docs, Some("Add two numbers together.".to_string()));
        assert!(item.signature.contains("def add(a: int, b: int) -> int"));
    }

    #[test]
    fn test_extract_class_and_methods() {
        let adapter = make_adapter();
        let source = r#"
class Person:
    """A person with a name."""

    def __init__(self, name: str):
        """Create a new person."""
        self.name = name

    def greet(self) -> str:
        """Return a greeting."""
        return f"Hello, I'm {self.name}"
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("person.py");

        let mut items = Vec::new();
        adapter.walk_module(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["person".to_string()],
            false,
            None,
            &mut items,
        );

        // Should have: class + __init__ + greet
        assert_eq!(items.len(), 3);

        let class_item = items.iter().find(|i| i.kind == DocKind::Class).unwrap();
        assert_eq!(class_item.name, "person.Person");

        let methods: Vec<_> = items.iter().filter(|i| i.kind == DocKind::Method).collect();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn test_skip_private_function() {
        let adapter = make_adapter();
        let source = r#"
def _private_helper():
    """Private helper."""
    pass
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.py");

        let mut items = Vec::new();
        adapter.walk_module(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false,
            None,
            &mut items,
        );

        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_adapter_interface() {
        let adapter = make_adapter();
        assert_eq!(adapter.name(), "python");
        assert_eq!(adapter.extensions(), &["py"]);
    }
}
