//! Rust adapter using tree-sitter for parsing.
//!
//! This adapter extracts documentation from Rust source files by:
//! 1. Parsing the source with tree-sitter-rust
//! 2. Walking the AST to find documented items (functions, structs, enums, etc.)
//! 3. Extracting doc comments and signatures
//! 4. Building markdown output files

use crate::{
    build_body, build_frontmatter, build_output_path, is_test_path, walk_source_files,
    AdapterError, AdapterOptions, AdapterOutput, DocAdapter, DocItem, DocKind, SourceLocation,
};
use std::path::Path;
use tracing::{debug, warn};
use tree_sitter::{Node, Parser, Tree};

/// Rust documentation adapter using tree-sitter
pub struct RustAdapter {
    parser: std::sync::Mutex<Parser>,
}

impl RustAdapter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to set Rust language for parser");
        Self {
            parser: std::sync::Mutex::new(parser),
        }
    }

    /// Parse source code into a tree-sitter Tree
    fn parse(&self, source: &str) -> Result<Tree, AdapterError> {
        let mut parser = self.parser.lock().map_err(|e| {
            AdapterError::ParseError(format!("Failed to acquire parser lock: {}", e))
        })?;
        parser
            .parse(source, None)
            .ok_or_else(|| AdapterError::ParseError("Failed to parse source".to_string()))
    }

    /// Extract all documented items from a single file
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

        // Derive module path from file path
        let module_path = derive_module_path(&rel_path);
        let include_private = options.get_bool("include_private", false);

        let mut items = Vec::new();
        let root = tree.root_node();

        // Walk top-level declarations
        self.walk_top_level(
            root,
            source_bytes,
            &rel_path,
            repo_url,
            &module_path,
            include_private,
            &mut items,
        );

        debug!(
            "Extracted {} items from {}",
            items.len(),
            path.display()
        );

        Ok(items)
    }

    /// Walk top-level declarations in a source file
    fn walk_top_level(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
        items: &mut Vec<DocItem>,
    ) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_item" => {
                    if let Some(item) = self.extract_function(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        None,
                        include_private,
                    ) {
                        items.push(item);
                    }
                }
                "struct_item" => {
                    if let Some(item) = self.extract_struct(
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
                "enum_item" => {
                    if let Some(item) = self.extract_enum(
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
                "trait_item" => {
                    if let Some(item) = self.extract_trait(
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
                "impl_item" => {
                    self.extract_impl_methods(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                        items,
                    );
                }
                "type_item" => {
                    if let Some(item) = self.extract_type_alias(
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
                "const_item" | "static_item" => {
                    if let Some(item) = self.extract_constant(
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
                "mod_item" => {
                    // Handle inline modules
                    self.extract_mod_item(
                        child,
                        source,
                        rel_path,
                        repo_url,
                        module_path,
                        include_private,
                        items,
                    );
                }
                _ => {}
            }
        }
    }

    /// Extract a function item
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
        // Get visibility
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        // Get function name
        let name_node = node.child_by_field_name("name")?;
        let func_name = node_text(name_node, source)?;

        // Skip test functions
        if func_name.starts_with("test_") || has_test_attribute(node, source) {
            return None;
        }

        // Get doc comments
        let docs = extract_doc_comments(node, source);

        // Build full name
        let full_name = build_full_name(module_path, container, &func_name);

        // Get signature (function header without body)
        let signature = extract_function_signature(node, source)?;

        // Get source code
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

    /// Extract a struct item
    fn extract_struct(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        let name_node = node.child_by_field_name("name")?;
        let struct_name = node_text(name_node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &struct_name);
        let signature = extract_struct_signature(node, source)?;
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: DocKind::Struct,
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

    /// Extract an enum item
    fn extract_enum(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        let name_node = node.child_by_field_name("name")?;
        let enum_name = node_text(name_node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &enum_name);
        let signature = extract_enum_signature(node, source)?;
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: DocKind::Enum,
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

    /// Extract a trait item
    fn extract_trait(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        let name_node = node.child_by_field_name("name")?;
        let trait_name = node_text(name_node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &trait_name);
        let item_source = node_text(node, source)?;

        // For traits, use the whole definition as signature
        let signature = item_source.clone();

        Some(DocItem {
            name: full_name,
            kind: DocKind::Trait,
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

    /// Extract a type alias
    fn extract_type_alias(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        let name_node = node.child_by_field_name("name")?;
        let type_name = node_text(name_node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &type_name);
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: DocKind::Type,
            docs,
            signature: item_source.clone(),
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

    /// Extract a constant or static item
    fn extract_constant(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
    ) -> Option<DocItem> {
        let is_pub = has_visibility(node, source);
        if !is_pub && !include_private {
            return None;
        }

        let name_node = node.child_by_field_name("name")?;
        let const_name = node_text(name_node, source)?;
        let docs = extract_doc_comments(node, source);
        let full_name = build_full_name(module_path, None, &const_name);
        let item_source = node_text(node, source)?;

        Some(DocItem {
            name: full_name,
            kind: DocKind::Constant,
            docs,
            signature: item_source.clone(),
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

    /// Extract methods from an impl block
    fn extract_impl_methods(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        module_path: &[String],
        include_private: bool,
        items: &mut Vec<DocItem>,
    ) {
        // Get the type being implemented
        let type_name = get_impl_type_name(node, source);

        // Find the declaration_list (impl body)
        let body = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return,
        };

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item" {
                if let Some(item) = self.extract_function(
                    child,
                    source,
                    rel_path,
                    repo_url,
                    module_path,
                    type_name.as_deref(),
                    include_private,
                ) {
                    items.push(item);
                }
            }
        }
    }

    /// Extract items from an inline module
    fn extract_mod_item(
        &self,
        node: Node,
        source: &[u8],
        rel_path: &Path,
        repo_url: Option<&str>,
        parent_module_path: &[String],
        include_private: bool,
        items: &mut Vec<DocItem>,
    ) {
        // Get module name
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let mod_name = match node_text(name_node, source) {
            Some(n) => n,
            None => return,
        };

        // Build new module path
        let mut new_module_path = parent_module_path.to_vec();
        new_module_path.push(mod_name);

        // Check for inline module body
        let body = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return, // External module declaration, skip
        };

        // Walk the module body
        self.walk_top_level(
            body,
            source,
            rel_path,
            repo_url,
            &new_module_path,
            include_private,
            items,
        );
    }
}

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DocAdapter for RustAdapter {
    fn name(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
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

            match self.extract_from_file(&path, source_root, repo_url, options) {
                Ok(items) => {
                    for item in items {
                        let frontmatter = build_frontmatter(&item, "rust");
                        let body_md = build_body(&item, "rust");
                        let output_rel_path = build_output_path(&item);

                        outputs.push(AdapterOutput {
                            output_rel_path,
                            frontmatter,
                            body_md,
                            source: item.location.clone(),
                            language: "rust".to_string(),
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

/// Get text content of a node
fn node_text(node: Node, source: &[u8]) -> Option<String> {
    std::str::from_utf8(&source[node.byte_range()]).ok().map(|s| s.to_string())
}

/// Derive module path from file path
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

    // Remove "lib" or "main" if they're the last component
    if let Some(last) = parts.last() {
        if last == "lib" || last == "main" {
            parts.pop();
        }
    }

    // Handle mod.rs - remove "mod" and use parent directory as module name
    if parts.last().map(|s| s.as_str()) == Some("mod") {
        parts.pop();
    }

    parts
}

/// Check if a node has pub visibility
fn has_visibility(node: Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            if let Some(text) = node_text(child, source) {
                return text.starts_with("pub");
            }
        }
    }
    false
}

/// Check if a function has #[test] attribute
fn has_test_attribute(node: Node, source: &[u8]) -> bool {
    // Look for attribute_item siblings before this node
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        let mut prev_sibling = None;
        for child in parent.children(&mut cursor) {
            if child.id() == node.id() {
                break;
            }
            if child.kind() == "attribute_item" {
                prev_sibling = Some(child);
            }
        }
        if let Some(attr) = prev_sibling {
            if let Some(text) = node_text(attr, source) {
                return text.contains("#[test]") || text.contains("#[cfg(test)]");
            }
        }
    }
    false
}

/// Extract doc comments preceding a node
fn extract_doc_comments(node: Node, source: &[u8]) -> Option<String> {
    let mut docs = Vec::new();

    // Look for preceding siblings that are comments
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        let mut prev_comments = Vec::new();

        for child in parent.children(&mut cursor) {
            if child.id() == node.id() {
                // Found our node, use collected comments
                docs = prev_comments;
                break;
            }

            if child.kind() == "line_comment" {
                if let Some(text) = node_text(child, source) {
                    // Check if it's a doc comment (/// or //!)
                    if text.starts_with("///") || text.starts_with("//!") {
                        let content = text
                            .trim_start_matches("///")
                            .trim_start_matches("//!")
                            .trim_start_matches(' ');
                        prev_comments.push(content.to_string());
                    }
                }
            } else if child.kind() == "block_comment" {
                if let Some(text) = node_text(child, source) {
                    // Check if it's a doc comment (/** or /*!)
                    if text.starts_with("/**") || text.starts_with("/*!") {
                        let content = text
                            .trim_start_matches("/**")
                            .trim_start_matches("/*!")
                            .trim_end_matches("*/")
                            .trim();
                        prev_comments.push(content.to_string());
                    }
                }
            } else if !matches!(child.kind(), "attribute_item" | "line_comment" | "block_comment") {
                // Non-comment, non-attribute node - reset collected comments
                prev_comments.clear();
            }
        }
    }

    if docs.is_empty() {
        None
    } else {
        // Join and trim trailing whitespace/newlines
        let result = docs.join("\n");
        let trimmed = result.trim_end();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// Build full name from module path, container, and item name
fn build_full_name(module_path: &[String], container: Option<&str>, name: &str) -> String {
    let mut parts: Vec<&str> = module_path.iter().map(|s| s.as_str()).collect();
    if let Some(c) = container {
        parts.push(c);
    }
    parts.push(name);
    parts.join("::")
}

/// Extract function signature (everything before the body)
fn extract_function_signature(node: Node, source: &[u8]) -> Option<String> {
    let full = node_text(node, source)?;

    // Find the body (block starting with {)
    if let Some(body) = node.child_by_field_name("body") {
        let body_start = body.start_byte();
        let sig_end = body_start - node.start_byte();
        let sig = &full[..sig_end];
        Some(sig.trim().to_string())
    } else {
        // No body (e.g., trait method declaration)
        Some(full.trim_end_matches(';').trim().to_string())
    }
}

/// Extract struct signature (just the header, not fields)
fn extract_struct_signature(node: Node, source: &[u8]) -> Option<String> {
    let full = node_text(node, source)?;

    // For struct with fields, extract just the declaration line
    if let Some(idx) = full.find('{') {
        Some(full[..idx].trim().to_string() + " { ... }")
    } else if let Some(idx) = full.find('(') {
        // Tuple struct
        Some(full[..idx].trim().to_string() + "(...)")
    } else {
        // Unit struct
        Some(full.trim_end_matches(';').trim().to_string())
    }
}

/// Extract enum signature
fn extract_enum_signature(node: Node, source: &[u8]) -> Option<String> {
    let full = node_text(node, source)?;

    if let Some(idx) = full.find('{') {
        Some(full[..idx].trim().to_string() + " { ... }")
    } else {
        Some(full.trim_end_matches(';').trim().to_string())
    }
}

/// Get the type name from an impl block
fn get_impl_type_name(node: Node, source: &[u8]) -> Option<String> {
    // Try to get the type field
    if let Some(type_node) = node.child_by_field_name("type") {
        // Extract just the type identifier, not generics
        return extract_type_identifier(type_node, source);
    }
    None
}

/// Extract type identifier, handling generics
fn extract_type_identifier(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node_text(node, source),
        "generic_type" => {
            // Get just the type name, not the generic arguments
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "type_identifier" {
                    return node_text(child, source);
                }
            }
            None
        }
        "scoped_type_identifier" => {
            // For paths like std::io::Error, get the last component
            let mut cursor = node.walk();
            let mut last_ident = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "type_identifier" {
                    last_ident = node_text(child, source);
                }
            }
            last_ident
        }
        _ => node_text(node, source),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_adapter() -> RustAdapter {
        RustAdapter::new()
    }

    #[test]
    fn test_derive_module_path_simple() {
        let path = PathBuf::from("src/config.rs");
        let result = derive_module_path(&path);
        assert_eq!(result, vec!["config"]);
    }

    #[test]
    fn test_derive_module_path_nested() {
        let path = PathBuf::from("src/markdown/parser.rs");
        let result = derive_module_path(&path);
        assert_eq!(result, vec!["markdown", "parser"]);
    }

    #[test]
    fn test_derive_module_path_lib() {
        let path = PathBuf::from("src/lib.rs");
        let result = derive_module_path(&path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_derive_module_path_mod() {
        let path = PathBuf::from("src/markdown/mod.rs");
        let result = derive_module_path(&path);
        assert_eq!(result, vec!["markdown"]);
    }

    #[test]
    fn test_derive_module_path_main() {
        let path = PathBuf::from("src/main.rs");
        let result = derive_module_path(&path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_simple_function() {
        let adapter = make_adapter();
        let source = r#"
/// This is a documented function.
/// It does something useful.
pub fn my_function(x: i32) -> i32 {
    x + 1
}
"#;
        let tree = adapter.parse(source).unwrap();
        let root = tree.root_node();

        // Verify we got a source_file with children
        assert_eq!(root.kind(), "source_file");
        assert!(root.child_count() > 0);
    }

    #[test]
    fn test_extract_public_function() {
        let adapter = make_adapter();
        let source = r#"
/// Adds two numbers together.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("math.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["math".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "math::add");
        assert_eq!(item.kind, DocKind::Function);
        assert_eq!(item.docs, Some("Adds two numbers together.".to_string()));
        assert!(item.signature.contains("pub fn add(a: i32, b: i32) -> i32"));
    }

    #[test]
    fn test_skip_private_function() {
        let adapter = make_adapter();
        let source = r#"
/// Private helper function.
fn private_helper() {}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false, // include_private = false
            &mut items,
        );

        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_include_private_function() {
        let adapter = make_adapter();
        let source = r#"
/// Private helper function.
fn private_helper() {}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            true, // include_private = true
            &mut items,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "private_helper");
    }

    #[test]
    fn test_extract_struct() {
        let adapter = make_adapter();
        let source = r#"
/// Configuration for the application.
pub struct Config {
    /// Path to the vault directory.
    pub vault: PathBuf,
    /// Output directory.
    pub output: PathBuf,
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("config.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["config".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "config::Config");
        assert_eq!(item.kind, DocKind::Struct);
        assert!(item.docs.as_ref().unwrap().contains("Configuration for the application"));
        assert!(item.signature.contains("pub struct Config"));
    }

    #[test]
    fn test_extract_enum() {
        let adapter = make_adapter();
        let source = r#"
/// Represents different note types.
pub enum NoteType {
    /// A regular note.
    Note,
    /// A document.
    Doc,
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("note.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["note".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "note::NoteType");
        assert_eq!(item.kind, DocKind::Enum);
    }

    #[test]
    fn test_extract_trait() {
        let adapter = make_adapter();
        let source = r#"
/// A trait for processing markdown.
pub trait MarkdownProcessor {
    /// Process the input text.
    fn process(&self, input: &str) -> String;
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("markdown.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["markdown".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "markdown::MarkdownProcessor");
        assert_eq!(item.kind, DocKind::Trait);
    }

    #[test]
    fn test_extract_impl_methods() {
        let adapter = make_adapter();
        let source = r#"
pub struct Config {
    pub name: String,
}

impl Config {
    /// Create a new Config.
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }

    /// Get the name.
    pub fn name(&self) -> &str {
        &self.name
    }
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("config.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["config".to_string()],
            false,
            &mut items,
        );

        // Should have: Config struct + 2 methods
        assert_eq!(items.len(), 3);

        let struct_item = items.iter().find(|i| i.kind == DocKind::Struct).unwrap();
        assert_eq!(struct_item.name, "config::Config");

        let methods: Vec<_> = items.iter().filter(|i| i.kind == DocKind::Method).collect();
        assert_eq!(methods.len(), 2);

        let new_method = methods.iter().find(|m| m.name.ends_with("::new")).unwrap();
        assert_eq!(new_method.name, "config::Config::new");
        assert_eq!(new_method.container, Some("Config".to_string()));
        assert_eq!(new_method.docs, Some("Create a new Config.".to_string()));
    }

    #[test]
    fn test_extract_type_alias() {
        let adapter = make_adapter();
        let source = r#"
/// A result type for our operations.
pub type Result<T> = std::result::Result<T, Error>;
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("error.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["error".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "error::Result");
        assert_eq!(item.kind, DocKind::Type);
    }

    #[test]
    fn test_extract_constant() {
        let adapter = make_adapter();
        let source = r#"
/// Default port for the server.
pub const DEFAULT_PORT: u16 = 8080;
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("server.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["server".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.name, "server::DEFAULT_PORT");
        assert_eq!(item.kind, DocKind::Constant);
    }

    #[test]
    fn test_skip_test_function() {
        let adapter = make_adapter();
        let source = r#"
pub fn real_function() {}

fn test_something() {}

#[test]
fn another_test() {}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            true, // include private to check test filtering
            &mut items,
        );

        // Only real_function should be included (tests filtered)
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "real_function");
    }

    #[test]
    fn test_multiline_doc_comments() {
        let adapter = make_adapter();
        let source = r#"
/// First line of documentation.
/// Second line of documentation.
/// Third line of documentation.
pub fn documented_function() {}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let docs = items[0].docs.as_ref().unwrap();
        assert!(docs.contains("First line"));
        assert!(docs.contains("Second line"));
        assert!(docs.contains("Third line"));
    }

    #[test]
    fn test_source_location() {
        let adapter = make_adapter();
        let source = r#"/// Doc
pub fn my_func() {
    // body
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("test.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            Some("https://github.com/user/repo"),
            &["test".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let loc = &items[0].location;
        assert_eq!(loc.file, PathBuf::from("test.rs"));
        assert!(loc.start_line.is_some());
        assert!(loc.end_line.is_some());
        assert_eq!(loc.repo_url, Some("https://github.com/user/repo".to_string()));

        let url = loc.to_url().unwrap();
        assert!(url.contains("github.com"));
        assert!(url.contains("test.rs"));
    }

    #[test]
    fn test_inline_module() {
        let adapter = make_adapter();
        let source = r#"
pub mod inner {
    /// Inner function.
    pub fn inner_func() {}
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("outer.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["outer".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "outer::inner::inner_func");
        assert_eq!(items[0].module_path, vec!["outer", "inner"]);
    }

    #[test]
    fn test_generic_struct() {
        let adapter = make_adapter();
        let source = r#"
/// A generic container.
pub struct Container<T> {
    value: T,
}

impl<T> Container<T> {
    /// Create a new container.
    pub fn new(value: T) -> Self {
        Self { value }
    }
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("container.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &["container".to_string()],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 2);

        let struct_item = items.iter().find(|i| i.kind == DocKind::Struct).unwrap();
        assert_eq!(struct_item.name, "container::Container");

        let method = items.iter().find(|i| i.kind == DocKind::Method).unwrap();
        assert_eq!(method.name, "container::Container::new");
        assert_eq!(method.container, Some("Container".to_string()));
    }

    #[test]
    fn test_adapter_interface() {
        let adapter = make_adapter();
        assert_eq!(adapter.name(), "rust");
        assert_eq!(adapter.extensions(), &["rs"]);
    }

    #[test]
    fn test_function_signature_extraction() {
        let adapter = make_adapter();
        let source = r#"
/// Complex function.
pub fn complex<T: Clone>(x: T, y: &str) -> Result<T, Error>
where
    T: Send + Sync,
{
    Ok(x.clone())
}
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        let sig = &items[0].signature;
        assert!(sig.contains("pub fn complex"));
        assert!(sig.contains("T: Clone"));
        assert!(sig.contains("Result<T, Error>"));
        assert!(sig.contains("where"));
        // Should NOT contain the body
        assert!(!sig.contains("Ok(x.clone())"));
    }

    #[test]
    fn test_tuple_struct() {
        let adapter = make_adapter();
        let source = r#"
/// A newtype wrapper.
pub struct Wrapper(pub String);
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, DocKind::Struct);
        assert!(items[0].signature.contains("pub struct Wrapper"));
    }

    #[test]
    fn test_unit_struct() {
        let adapter = make_adapter();
        let source = r#"
/// A marker type.
pub struct Marker;
"#;
        let tree = adapter.parse(source).unwrap();
        let source_bytes = source.as_bytes();
        let rel_path = PathBuf::from("lib.rs");

        let mut items = Vec::new();
        adapter.walk_top_level(
            tree.root_node(),
            source_bytes,
            &rel_path,
            None,
            &[],
            false,
            &mut items,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, DocKind::Struct);
        assert!(items[0].signature.contains("pub struct Marker"));
    }

    #[test]
    fn test_extract_from_temp_file() {
        use tempfile::TempDir;
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let source = r#"
/// A test struct.
pub struct TestStruct {
    pub field: String,
}

impl TestStruct {
    /// Creates a new TestStruct.
    pub fn new(field: &str) -> Self {
        Self { field: field.to_string() }
    }
}
"#;
        fs::write(&file_path, source).unwrap();

        let adapter = make_adapter();
        let options = AdapterOptions::new();
        let outputs = adapter.extract(temp_dir.path(), None, &options).unwrap();

        assert_eq!(outputs.len(), 2); // struct + method

        let struct_out = outputs.iter().find(|o| o.kind == DocKind::Struct).unwrap();
        assert!(struct_out.frontmatter.title.contains("TestStruct"));
        assert!(struct_out.body_md.contains("A test struct"));

        let method_out = outputs.iter().find(|o| o.kind == DocKind::Method).unwrap();
        assert!(method_out.frontmatter.title.contains("new"));
    }

    #[test]
    fn test_adapter_output_to_markdown() {
        use tempfile::TempDir;
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("lib.rs");

        let source = r#"
/// Adds one to a number.
pub fn add_one(x: i32) -> i32 {
    x + 1
}
"#;
        fs::write(&file_path, source).unwrap();

        let adapter = make_adapter();
        let options = AdapterOptions::new();
        let outputs = adapter.extract(temp_dir.path(), None, &options).unwrap();

        assert_eq!(outputs.len(), 1);

        let md = outputs[0].to_markdown().unwrap();
        assert!(md.starts_with("---\n"));
        assert!(md.contains("title:"));
        assert!(md.contains("add_one"));
        assert!(md.contains("Adds one to a number"));
    }
}
