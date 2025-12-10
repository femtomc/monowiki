//! Zig adapter using tree-sitter for parsing.
//!
//! Extracts documentation from Zig source files including:
//! - Functions (pub fn, fn)
//! - Structs, Enums, Unions
//! - Constants and variables
//! - Test declarations (optional)

use crate::{
    build_body, build_frontmatter, build_output_path, is_test_path, walk_source_files,
    AdapterError, AdapterOptions, AdapterOutput, DocAdapter, DocItem, DocKind, SourceLocation,
};
use std::path::Path;
use tracing::{debug, warn};
use tree_sitter::{Node, Parser, Tree};

/// Zig documentation adapter using tree-sitter
pub struct ZigAdapter {
    parser: std::sync::Mutex<Parser>,
}

impl ZigAdapter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_zig::LANGUAGE.into())
            .expect("Failed to set Zig language for parser");
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

        self.walk_declarations(
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

    fn walk_declarations(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
        container: Option<&str>,
        items: &mut Vec<DocItem>,
    ) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "FnDecl" => {
                    if let Some(item) = self.extract_function(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        container,
                        include_private,
                    ) {
                        items.push(item);
                    }
                }
                "VarDecl" => {
                    if let Some(item) = self.extract_var_decl(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                    ) {
                        items.push(item);
                    }
                }
                "TopLevelDecl" => {
                    // Recurse into top-level declarations
                    self.walk_declarations(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                        container,
                        items,
                    );
                }
                "Decl" => {
                    // Generic declaration wrapper - recurse
                    self.walk_declarations(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                        container,
                        items,
                    );
                }
                "ContainerDecl" | "ContainerDeclAuto" => {
                    // This could be a struct, enum, or union
                    if let Some((type_item, type_name)) = self.extract_container_type(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                    ) {
                        items.push(type_item);

                        // Walk the container's members
                        self.walk_declarations(
                            child,
                            source,
                            rel_path,
                            repo_url,
                            module_path,
                            include_private,
                            Some(&type_name),
                            items,
                        );
                    }
                }
                _ => {
                    // Recurse to find nested declarations
                    self.walk_declarations(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                        container,
                        items,
                    );
                }
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
        container: Option<&str>,
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = is_public(node, source);
        if !is_pub && !include_private {
            return None;
        }

        // Find the function name
        let func_name = find_fn_name(node, source)?;

        // Skip test functions
        if func_name.starts_with("test") {
            return None;
        }

        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, container, &func_name);
        let signature = extract_fn_signature(node, source)?;
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: if container.is_some() {
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
            container: container.map(String::from),
        })
    }

    fn extract_var_decl(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = is_public(node, source);
        if !is_pub && !include_private {
            return None;
        }

        // Find variable/const name
        let var_name = find_var_name(node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &var_name);
        let signature = node_text(node, source)?.lines().next()?.to_string();
        let item_source = node_text(node, source)?;

        // Check if this is a type definition (struct, enum, etc.)
        let kind = if is_type_definition(node, source) {
            DocKind::Type
        } else {
            DocKind::Constant
        };

        Some(DocItem {
            name: full_name,
            kind,
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
        })
    }

    fn extract_container_type(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        _include_private: bool,
    ) -> Option<(DocItem, String)> {
        // Container types in Zig are typically assigned to const variables
        // The parent VarDecl has the name
        let parent = node.parent()?;
        if parent.kind() != "VarDecl" {
            return None;
        }

        let type_name = find_var_name(parent, source)?;
        let docs = extract_doc_comments(parent, source);
        let full_name = build_full_name(module_path, None, &type_name);

        // Determine if it's struct, enum, or union
        let kind = determine_container_kind(node, source);
        let signature = extract_container_signature(node, source, &type_name)?;
        let item_source = node_text(parent, source)?;

        let item = DocItem {
            name: full_name,
            kind,
            docs,
            signature,
            source: Some(item_source),
            location: SourceLocation {
                file: rel_path.to_path_buf(),
                start_line: Some(parent.start_position().row as u32 + 1),
                end_line: Some(parent.end_position().row as u32 + 1),
                repo_url: repo_url.map(String::from),
            },
            module_path: module_path.to_vec(),
            container: None,
        };

        Some((item, type_name))
    }
}

impl Default for ZigAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DocAdapter for ZigAdapter {
    fn name(&self) -> &str {
        "zig"
    }

    fn extensions(&self) -> &[&str] {
        &["zig"]
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
            if skip_tests && is_test_path(&path) {
                debug!("Skipping test file: {}", path.display());
                continue;
            }

            // Skip zig-cache and zig-out directories
            if path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.starts_with("zig-cache") || s.starts_with("zig-out") || s.starts_with(".")
            }) {
                continue;
            }

            match self.extract_from_file(&path, source_root, repo_url, options) {
                Ok(items) => {
                    for item in items {
                        let frontmatter = build_frontmatter(&item, "zig");
                        let body_md = build_body(&item, "zig");
                        let output_rel_path = build_output_path(&item);

                        outputs.push(AdapterOutput {
                            output_rel_path,
                                frontmatter,
                            body_md,
                            source: item.location.clone(),
                            language: "zig".to_string(),
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

    // Remove "src" if it's the first component
    if parts.first().map(|s| s.as_str()) == Some("src") {
        parts.remove(0);
    }

    parts
}

fn build_full_name(module_path: &[String], container: Option<&str>, name: &str) -> String {
    let mut parts: Vec<&str> = module_path.iter().map(|s| s.as_str()).collect();
    if let Some(c) = container {
        parts.push(c);
    }
    parts.push(name);
    parts.join(".")
}

fn is_public(node: Node, source: &[u8]) -> bool {
    // Check for "pub" keyword in the node or its siblings
    let text = node_text(node, source).unwrap_or_default();
    text.trim_start().starts_with("pub ")
}

fn find_fn_name(node: Node, source: &[u8]) -> Option<String> {
    // Look for IDENTIFIER child that is the function name
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "IDENTIFIER" {
            return node_text(child, source);
        }
        // Also check in FnProto
        if child.kind() == "FnProto" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "IDENTIFIER" {
                    return node_text(inner, source);
                }
            }
        }
    }
    None
}

fn find_var_name(node: Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "IDENTIFIER" {
            return node_text(child, source);
        }
    }
    None
}

fn is_type_definition(node: Node, source: &[u8]) -> bool {
    let text = node_text(node, source).unwrap_or_default();
    text.contains("struct") || text.contains("enum") || text.contains("union")
}

fn determine_container_kind(node: Node, source: &[u8]) -> DocKind {
    let text = node_text(node, source).unwrap_or_default();
    if text.contains("enum") {
        DocKind::Enum
    } else if text.contains("union") {
        DocKind::Type // Using Type for unions
    } else {
        DocKind::Struct
    }
}

fn extract_doc_comments(node: Node, source: &[u8]) -> Option<String> {
    // In Zig, doc comments are /// lines preceding the declaration
    let start_line = node.start_position().row;
    if start_line == 0 {
        return None;
    }

    let source_str = std::str::from_utf8(source).ok()?;
    let lines: Vec<&str> = source_str.lines().collect();

    let mut doc_lines = Vec::new();
    let mut line_idx = start_line.saturating_sub(1);

    // Walk backwards to collect doc comments
    while line_idx > 0 {
        let line = lines.get(line_idx)?;
        let trimmed = line.trim();

        if trimmed.starts_with("///") {
            let content = trimmed
                .trim_start_matches("///")
                .trim_start_matches(' ');
            doc_lines.push(content.to_string());
            line_idx = line_idx.saturating_sub(1);
        } else if trimmed.is_empty() {
            // Allow empty lines in doc comments
            line_idx = line_idx.saturating_sub(1);
        } else {
            break;
        }
    }

    // Also check the line immediately before (line_idx might be 0)
    if let Some(line) = lines.get(line_idx) {
        let trimmed = line.trim();
        if trimmed.starts_with("///") {
            let content = trimmed
                .trim_start_matches("///")
                .trim_start_matches(' ');
            doc_lines.push(content.to_string());
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        doc_lines.reverse();
        Some(doc_lines.join("\n"))
    }
}

fn extract_fn_signature(node: Node, source: &[u8]) -> Option<String> {
    let text = node_text(node, source)?;

    // Find the opening brace and take everything before it
    if let Some(idx) = text.find('{') {
        Some(text[..idx].trim().to_string())
    } else {
        // No body (maybe extern fn)
        Some(text.lines().next()?.trim().to_string())
    }
}

fn extract_container_signature(node: Node, source: &[u8], name: &str) -> Option<String> {
    let text = node_text(node, source)?;

    // Find what kind of container this is
    let kind = if text.contains("enum") {
        "enum"
    } else if text.contains("union") {
        "union"
    } else {
        "struct"
    };

    Some(format!("const {} = {} {{ ... }}", name, kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_adapter() -> ZigAdapter {
        ZigAdapter::new()
    }

    #[test]
    fn test_parse_simple() {
        let adapter = make_adapter();
        let source = r#"
pub fn add(a: i32, b: i32) i32 {
    return a + b;
}
"#;
        let tree = adapter.parse(source).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
    }

    #[test]
    fn test_adapter_interface() {
        let adapter = make_adapter();
        assert_eq!(adapter.name(), "zig");
        assert_eq!(adapter.extensions(), &["zig"]);
    }
}
