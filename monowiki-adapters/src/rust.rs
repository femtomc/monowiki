use crate::DocMeta;
use crate::{AdapterError, AdapterOptions, AdapterOutput, DocAdapter, SourceMeta};
use monowiki_core::Frontmatter;
use proc_macro2::Span;
use quote::ToTokens;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use walkdir::WalkDir;

/// Extracts Rust `///` and `//!` docs into markdown pages.
pub struct RustDocAdapter;

#[derive(Debug, Clone)]
struct SourceSnippet {
    code: String,
    start_line: u32,
    end_line: u32,
}

impl RustDocAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl DocAdapter for RustDocAdapter {
    fn name(&self) -> &str {
        "rust"
    }

    fn extract(
        &self,
        source_root: &Path,
        repo_url: Option<&str>,
        options: &AdapterOptions,
    ) -> Result<Vec<AdapterOutput>, AdapterError> {
        let include_private = options.get_bool("include_private", false);
        let include_tests = options.get_bool("include_tests", false);
        let include_undocumented = options.get_bool("include_undocumented", false);
        let include_modules = options.get_bool("include_modules", false);
        let _number_snippets = options.get_bool("numbered_snippets", false);
        let repo_prefix = options
            .get_string("repo_path_prefix")
            .map(PathBuf::from)
            .unwrap_or_default();

        let mut outputs = Vec::new();
        let mut module_docs_generated = 0usize;

        for entry in WalkDir::new(source_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            if !include_tests && is_test_path(path) {
                continue;
            }

            let rel_path = path.strip_prefix(source_root).unwrap_or(path).to_path_buf();
            let repo_rel = if repo_prefix.as_os_str().is_empty() {
                rel_path.clone()
            } else {
                repo_prefix.join(&rel_path)
            };
            let module_path = module_path_from_rel(&rel_path);
            let module_parts = module_parts_from_rel(&rel_path);

            let src = fs::read_to_string(path)?;
            let file =
                syn::parse_file(&src).map_err(|e| AdapterError::ParseError(e.to_string()))?;

            // Module-level docs (//!) if enabled
            if include_modules {
                if let Some((module_docs, module_line)) = doc_from_attrs_with_line(&file.attrs) {
                    let module_name = module_name_from_rel(&rel_path);
                    let qualified = if module_parts.is_empty() {
                        module_name.clone()
                    } else {
                        module_parts.join("::")
                    };
                    let out_path = module_output_path(&module_path);
                    outputs.push(AdapterOutput {
                        output_rel_path: out_path,
                        frontmatter: build_frontmatter(
                            &qualified,
                            DocKind::Module,
                            Some(module_docs.as_str()),
                        ),
                        body_md: render_body(
                            &qualified,
                            &format!("mod {}", qualified),
                            Some(module_docs.as_str()),
                            &repo_rel,
                            repo_url,
                            module_line,
                            DocKind::Module,
                            None,
                            "rust",
                        ),
                        source: SourceMeta {
                            file: repo_rel.clone(),
                            line: module_line,
                            repo_url: repo_url.map(|s| s.to_string()),
                        },
                        meta: DocMeta {
                            language: "rust".to_string(),
                            kind: DocKind::Module.tag().to_string(),
                            module_path: module_parts.clone(),
                            container: None,
                        },
                    });
                    tracing::info!(module = %qualified, path = ?module_output_path(&module_path), "module doc generated");
                    module_docs_generated += 1;
                }
            }

            process_items(
                file.items,
                &module_parts,
                &module_path,
                &repo_rel,
                repo_url,
                include_private,
                include_undocumented,
                include_modules,
                &mut outputs,
                &mut module_docs_generated,
                &src,
            );
        }

        tracing::info!(
            modules = module_docs_generated,
            total = outputs.len(),
            "rust adapter extracted docs"
        );
        Ok(outputs)
    }
}

fn doc_from_attrs_with_line(attrs: &[syn::Attribute]) -> Option<(String, Option<u32>)> {
    let mut lines = Vec::new();
    let mut line = None;
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(meta) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit),
                ..
            }) = &meta.value
            {
                if line.is_none() {
                    line = line_from_span(meta.path.span());
                }
                let value = lit.value();
                let cleaned = value.strip_prefix(' ').unwrap_or(&value).to_string();
                lines.push(cleaned);
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        let joined = lines.join("\n");
        let cleaned = joined.trim_end_matches('\n').to_string();
        if cleaned.is_empty() {
            None
        } else {
            Some((cleaned, line))
        }
    }
}

fn visibility_allows(vis: &syn::Visibility, include_private: bool) -> bool {
    include_private || matches!(vis, syn::Visibility::Public(_))
}

fn is_test_path(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "__tests__")
        || path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with("_test") || s.ends_with("_tests"))
            .unwrap_or(false)
}

fn module_path_from_rel(rel: &Path) -> PathBuf {
    let mut parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if let Some(last) = parts.pop() {
        if last != "mod.rs" && last != "lib.rs" && last != "main.rs" {
            parts.push(last.trim_end_matches(".rs").to_string());
        }
    }
    parts.into_iter().collect()
}

fn module_parts_from_rel(rel: &Path) -> Vec<String> {
    let mut parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if let Some(last) = parts.pop() {
        if last != "mod.rs" && last != "lib.rs" && last != "main.rs" {
            parts.push(last.trim_end_matches(".rs").to_string());
        }
    }
    parts
}

fn module_name_from_rel(rel: &Path) -> String {
    if let Some(file_name) = rel.file_name().and_then(|s| s.to_str()) {
        if file_name == "lib.rs" {
            return "crate".to_string();
        }
        if file_name == "main.rs" {
            return "main".to_string();
        }
        if file_name == "mod.rs" {
            if let Some(parent) = rel
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
            {
                return parent.to_string();
            }
        }
        return file_name.trim_end_matches(".rs").to_string();
    }
    "module".to_string()
}

fn qualified_name(module_parts: &[String], ident: &str) -> String {
    if module_parts.is_empty() {
        ident.to_string()
    } else {
        format!("{}::{}", module_parts.join("::"), ident)
    }
}

fn qualified_method_name(module_parts: &[String], type_name: &str, method: &str) -> String {
    let mut prefix = if module_parts.is_empty() {
        String::new()
    } else {
        format!("{}::", module_parts.join("::"))
    };
    prefix.push_str(type_name);
    prefix.push_str("::");
    prefix.push_str(method);
    prefix
}

fn module_tag_from_title(title: &str, kind: DocKind) -> Option<String> {
    let mut parts: Vec<&str> = title.split("::").collect();
    if parts.is_empty() {
        return None;
    }
    if kind != DocKind::Module && parts.len() > 1 {
        parts.pop();
    }
    let module = parts.join("::");
    if module.is_empty() {
        None
    } else {
        Some(format!("module:{}", module))
    }
}

fn output_path(module_path: &Path, name: &str, parent: Option<&str>) -> PathBuf {
    let mut path = module_path.to_path_buf();
    if let Some(parent) = parent {
        path.push(parent);
    }
    path.push(format!("{}.md", name));
    path
}

fn module_output_path(module_path: &Path) -> PathBuf {
    let mut path = module_path.to_path_buf();
    path.push("module.md");
    path
}

fn build_frontmatter(title: &str, kind: DocKind, docs: Option<&str>) -> Frontmatter {
    let module_tag = module_tag_from_title(title, kind);
    let summary = docs.and_then(|d| {
        d.lines()
            .find(|l| !l.trim().is_empty())
            .map(|s| s.to_string())
    });
    let mut aliases = Vec::new();
    aliases.push(title.to_string());
    let mut tags = vec![
        "rust".to_string(),
        "api".to_string(),
        format!("kind:{}", kind.tag()),
    ];
    if let Some(mt) = module_tag {
        tags.push(mt);
    }
    let slug = monowiki_core::slugify(&title.replace("::", "-"));
    Frontmatter {
        title: title.to_string(),
        slug: Some(slug),
        note_type: Some("doc".to_string()),
        tags,
        aliases,
        summary,
        ..Default::default()
    }
}

fn render_body(
    title: &str,
    signature: &str,
    docs: Option<&str>,
    source_rel: &Path,
    repo_url: Option<&str>,
    line: Option<u32>,
    kind: DocKind,
    source_snippet: Option<&SourceSnippet>,
    lang: &str,
) -> String {
    let mut body = String::new();
    body.push_str("# ");
    body.push_str(title);
    body.push_str("\n\n");

    let (source_display, source_href) = source_link(source_rel, repo_url, line);
    body.push_str("<div class=\"doc-header\">\n");
    body.push_str(&format!(
        "  <span class=\"doc-kind-badge\">{}</span>\n",
        kind.display()
    ));
    if let Some(url) = source_href {
        body.push_str(&format!(
            "  <span class=\"doc-source-link\">Source: <a href=\"{}\">{}</a></span>\n",
            url, source_display
        ));
    } else {
        body.push_str(&format!(
            "  <span class=\"doc-source-link\">Source: {}</span>\n",
            source_display
        ));
    }
    body.push_str("</div>\n\n");

    if !signature.trim().is_empty() {
        body.push_str("<div class=\"code-block doc-signature\">\n");
        body.push_str("  <div class=\"code-toolbar\"><span class=\"code-title\">Signature</span><button class=\"copy-code-btn\" type=\"button\" aria-label=\"Copy signature\">Copy</button></div>\n");
        body.push_str("  <pre><code class=\"language-");
        body.push_str(lang);
        body.push_str("\">");
        body.push_str(&escape_html(&normalize_sig(signature)));
        body.push_str("</code></pre>\n");
        body.push_str("</div>\n\n");
    }

    if let Some(d) = docs {
        body.push_str(d.trim());
        body.push_str("\n\n");
    }

    if let Some(snippet) = source_snippet {
        let path_display = normalize_rel_display(source_rel);
        let range = format!("L{}–L{}", snippet.start_line, snippet.end_line);
        let link = source_link(source_rel, repo_url, Some(snippet.start_line)).1;
        body.push_str("<details class=\"doc-source-snippet\" open>\n");
        body.push_str("  <summary>");
        if let Some(url) = link {
            body.push_str(&format!(
                "<a href=\"{}\">{} {}</a>",
                url, path_display, range
            ));
        } else {
            body.push_str(&format!("{} {}", path_display, range));
        }
        body.push_str("</summary>\n");
        body.push_str("  <div class=\"code-block\">\n");
        body.push_str("    <div class=\"code-toolbar\"><span class=\"code-title\">Reference</span><button class=\"copy-code-btn\" type=\"button\" aria-label=\"Copy reference source\">Copy</button></div>\n");
        body.push_str("    <pre><code class=\"language-");
        body.push_str(lang);
        body.push_str("\">");
        let numbered = number_snippet(&snippet.code, snippet.start_line);
        body.push_str(&escape_html(&numbered));
        body.push_str("</code></pre>\n");
        body.push_str("  </div>\n");
        body.push_str("</details>\n");
    }

    body
}

fn source_link(
    rel_path: &Path,
    repo_url: Option<&str>,
    line: Option<u32>,
) -> (String, Option<String>) {
    let rel_display = normalize_rel_display(rel_path);
    let line_suffix = line.map(|l| format!("#L{}", l)).unwrap_or_default();
    if let Some(repo) = repo_url {
        let trimmed = repo.trim_end_matches('/');
        let has_blob_or_tree = trimmed.contains("/blob/") || trimmed.contains("/tree/");
        let base = if has_blob_or_tree {
            trimmed.to_string()
        } else {
            format!("{}/blob/main", trimmed)
        };
        let url = format!("{}/{}{}", base, rel_display, line_suffix);
        (rel_display, Some(url))
    } else {
        (rel_display, None)
    }
}

fn normalize_rel_display(rel_path: &Path) -> String {
    rel_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn vis_prefix(vis: &syn::Visibility) -> String {
    let v = vis.to_token_stream().to_string();
    if v.is_empty() {
        String::new()
    } else {
        format!("{} ", v)
    }
}

fn self_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|seg| seg.ident.to_string()),
        _ => None,
    }
}

fn line_from_span(span: Span) -> Option<u32> {
    let start = span.start();
    if start.line == 0 {
        None
    } else {
        Some(start.line as u32)
    }
}

fn source_snippet(src: &str, span: Span) -> Option<SourceSnippet> {
    let start = span.start();
    let end = span.end();
    if start.line == 0 || end.line == 0 {
        return None;
    }

    let lines: Vec<&str> = src.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let start_idx = start.line.saturating_sub(1) as usize;
    let end_idx = end.line.saturating_sub(1) as usize;

    let from = start_idx.min(lines.len().saturating_sub(1));
    let to = end_idx.min(lines.len().saturating_sub(1));
    let snippet = lines[from..=to].join("\n");

    Some(SourceSnippet {
        code: snippet,
        start_line: (from + 1) as u32,
        end_line: (to + 1) as u32,
    })
}

fn number_snippet(snippet: &str, start_line: u32) -> String {
    let line_count = snippet.lines().count() as u32;
    let width = (start_line + line_count).to_string().len();
    snippet
        .lines()
        .enumerate()
        .map(|(idx, line)| {
            format!(
                "{:>width$} | {}",
                start_line + idx as u32,
                line,
                width = width
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_sig(sig: &str) -> String {
    sig.replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" ,", ",")
        .replace(" :", ":")
        .replace(" ,)", ")")
        .replace(" ->", " ->")
        .replace("-> ", "-> ")
        .replace("< ", "<")
        .replace(" <", "<")
        .replace(" >", ">")
        .replace("> ,", ">,")
        .replace(", <", ",<")
        .replace("& self", "&self")
        .replace("& mut self", "&mut self")
        .replace("& ", "&")
        .replace(" &", " &")
}

fn render_method_signature(fun: &syn::ImplItemFn) -> String {
    let raw = format!("{}{}", vis_prefix(&fun.vis), fun.sig.to_token_stream());
    normalize_sig(&raw)
}

#[derive(Clone, Copy, PartialEq)]
enum DocKind {
    Module,
    Function,
    Struct,
    Enum,
    Trait,
    Method,
}

impl DocKind {
    fn display(&self) -> &'static str {
        match self {
            DocKind::Module => "Module",
            DocKind::Function => "Function",
            DocKind::Struct => "Struct",
            DocKind::Enum => "Enum",
            DocKind::Trait => "Trait",
            DocKind::Method => "Method",
        }
    }

    fn tag(&self) -> &'static str {
        match self {
            DocKind::Module => "module",
            DocKind::Function => "function",
            DocKind::Struct => "struct",
            DocKind::Enum => "enum",
            DocKind::Trait => "trait",
            DocKind::Method => "method",
        }
    }
}

fn process_items(
    items: Vec<syn::Item>,
    module_parts: &[String],
    module_path: &Path,
    repo_rel: &Path,
    repo_url: Option<&str>,
    include_private: bool,
    include_undocumented: bool,
    include_modules: bool,
    outputs: &mut Vec<AdapterOutput>,
    module_docs_generated: &mut usize,
    src: &str,
) {
    for item in items {
        match item {
            syn::Item::Fn(item_fn) => {
                if !visibility_allows(&item_fn.vis, include_private) {
                    continue;
                }
                let docs = doc_from_attrs_with_line(&item_fn.attrs);
                if docs.is_none() && !include_undocumented {
                    continue;
                }
                let signature = format!(
                    "{}{}",
                    vis_prefix(&item_fn.vis),
                    item_fn.sig.to_token_stream()
                );
                let qualified = qualified_name(&module_parts, &item_fn.sig.ident.to_string());
                let out_path = output_path(&module_path, &item_fn.sig.ident.to_string(), None);
                let line = docs
                    .as_ref()
                    .and_then(|(_, l)| *l)
                    .or_else(|| line_from_span(item_fn.sig.ident.span()));
                let snippet = source_snippet(src, item_fn.span());
                let docs_text = docs.as_ref().map(|(d, _)| d.as_str());
                outputs.push(AdapterOutput {
                    output_rel_path: out_path,
                    frontmatter: build_frontmatter(&qualified, DocKind::Function, docs_text),
                    body_md: render_body(
                        &qualified,
                        &signature,
                        docs_text,
                        repo_rel,
                        repo_url,
                        line,
                        DocKind::Function,
                        snippet.as_ref(),
                        "rust",
                    ),
                    source: SourceMeta {
                        file: repo_rel.to_path_buf(),
                        line,
                        repo_url: repo_url.map(|s| s.to_string()),
                    },
                    meta: DocMeta {
                        language: "rust".to_string(),
                        kind: DocKind::Function.tag().to_string(),
                        module_path: module_parts.to_vec(),
                        container: None,
                    },
                });
            }
            syn::Item::Struct(item_struct) => {
                if !visibility_allows(&item_struct.vis, include_private) {
                    continue;
                }
                let docs = doc_from_attrs_with_line(&item_struct.attrs);
                if docs.is_none() && !include_undocumented {
                    continue;
                }
                let sig = format!(
                    "{}struct {}{}",
                    vis_prefix(&item_struct.vis),
                    item_struct.ident,
                    item_struct.generics.to_token_stream()
                );
                let qualified = qualified_name(&module_parts, &item_struct.ident.to_string());
                let out_path = output_path(&module_path, &item_struct.ident.to_string(), None);
                let line = docs
                    .as_ref()
                    .and_then(|(_, l)| *l)
                    .or_else(|| line_from_span(item_struct.ident.span()));
                let snippet = source_snippet(src, item_struct.span());
                let docs_text = docs.as_ref().map(|(d, _)| d.as_str());
                outputs.push(AdapterOutput {
                    output_rel_path: out_path,
                    frontmatter: build_frontmatter(&qualified, DocKind::Struct, docs_text),
                    body_md: render_body(
                        &qualified,
                        &sig,
                        docs_text,
                        repo_rel,
                        repo_url,
                        line,
                        DocKind::Struct,
                        snippet.as_ref(),
                        "rust",
                    ),
                    source: SourceMeta {
                        file: repo_rel.to_path_buf(),
                        line,
                        repo_url: repo_url.map(|s| s.to_string()),
                    },
                    meta: DocMeta {
                        language: "rust".to_string(),
                        kind: DocKind::Struct.tag().to_string(),
                        module_path: module_parts.to_vec(),
                        container: None,
                    },
                });
            }
            syn::Item::Enum(item_enum) => {
                if !visibility_allows(&item_enum.vis, include_private) {
                    continue;
                }
                let docs = doc_from_attrs_with_line(&item_enum.attrs);
                if docs.is_none() && !include_undocumented {
                    continue;
                }
                let sig = format!(
                    "{}enum {}{}",
                    vis_prefix(&item_enum.vis),
                    item_enum.ident,
                    item_enum.generics.to_token_stream()
                );
                let qualified = qualified_name(&module_parts, &item_enum.ident.to_string());
                let out_path = output_path(&module_path, &item_enum.ident.to_string(), None);
                let line = docs
                    .as_ref()
                    .and_then(|(_, l)| *l)
                    .or_else(|| line_from_span(item_enum.ident.span()));
                let snippet = source_snippet(src, item_enum.span());
                let docs_text = docs.as_ref().map(|(d, _)| d.as_str());
                outputs.push(AdapterOutput {
                    output_rel_path: out_path,
                    frontmatter: build_frontmatter(&qualified, DocKind::Enum, docs_text),
                    body_md: render_body(
                        &qualified,
                        &sig,
                        docs_text,
                        repo_rel,
                        repo_url,
                        line,
                        DocKind::Enum,
                        snippet.as_ref(),
                        "rust",
                    ),
                    source: SourceMeta {
                        file: repo_rel.to_path_buf(),
                        line,
                        repo_url: repo_url.map(|s| s.to_string()),
                    },
                    meta: DocMeta {
                        language: "rust".to_string(),
                        kind: DocKind::Enum.tag().to_string(),
                        module_path: module_parts.to_vec(),
                        container: None,
                    },
                });
            }
            syn::Item::Trait(item_trait) => {
                if !visibility_allows(&item_trait.vis, include_private) {
                    continue;
                }
                let docs = doc_from_attrs_with_line(&item_trait.attrs);
                if docs.is_none() && !include_undocumented {
                    continue;
                }
                let sig = format!(
                    "{}trait {}{}",
                    vis_prefix(&item_trait.vis),
                    item_trait.ident,
                    item_trait.generics.to_token_stream()
                );
                let qualified = qualified_name(&module_parts, &item_trait.ident.to_string());
                let out_path = output_path(&module_path, &item_trait.ident.to_string(), None);
                let line = docs
                    .as_ref()
                    .and_then(|(_, l)| *l)
                    .or_else(|| line_from_span(item_trait.ident.span()));
                let snippet = source_snippet(src, item_trait.span());
                let docs_text = docs.as_ref().map(|(d, _)| d.as_str());
                outputs.push(AdapterOutput {
                    output_rel_path: out_path,
                    frontmatter: build_frontmatter(&qualified, DocKind::Trait, docs_text),
                    body_md: render_body(
                        &qualified,
                        &sig,
                        docs_text,
                        repo_rel,
                        repo_url,
                        line,
                        DocKind::Trait,
                        snippet.as_ref(),
                        "rust",
                    ),
                    source: SourceMeta {
                        file: repo_rel.to_path_buf(),
                        line,
                        repo_url: repo_url.map(|s| s.to_string()),
                    },
                    meta: DocMeta {
                        language: "rust".to_string(),
                        kind: DocKind::Trait.tag().to_string(),
                        module_path: module_parts.to_vec(),
                        container: None,
                    },
                });
            }
            syn::Item::Impl(item_impl) => {
                let Some(type_name) = self_type_name(&item_impl.self_ty) else {
                    continue;
                };
                for impl_item in item_impl.items {
                    if let syn::ImplItem::Fn(fun) = impl_item {
                        if !visibility_allows(&fun.vis, include_private) {
                            continue;
                        }
                        let docs = doc_from_attrs_with_line(&fun.attrs);
                        if docs.is_none() && !include_undocumented {
                            continue;
                        }
                        let qualified = qualified_method_name(
                            &module_parts,
                            &type_name,
                            &fun.sig.ident.to_string(),
                        );
                        let sig = render_method_signature(&fun);
                        let out_path =
                            output_path(&module_path, &fun.sig.ident.to_string(), Some(&type_name));
                        let line = docs
                            .as_ref()
                            .and_then(|(_, l)| *l)
                            .or_else(|| line_from_span(fun.sig.ident.span()));
                        let snippet = source_snippet(src, fun.span());
                        let docs_text = docs.as_ref().map(|(d, _)| d.as_str());
                        outputs.push(AdapterOutput {
                            output_rel_path: out_path,
                            frontmatter: build_frontmatter(&qualified, DocKind::Method, docs_text),
                            body_md: render_body(
                                &qualified,
                                &sig,
                                docs_text,
                                repo_rel,
                                repo_url,
                                line,
                                DocKind::Method,
                                snippet.as_ref(),
                                "rust",
                            ),
                            source: SourceMeta {
                                file: repo_rel.to_path_buf(),
                                line,
                                repo_url: repo_url.map(|s| s.to_string()),
                            },
                            meta: DocMeta {
                                language: "rust".to_string(),
                                kind: DocKind::Method.tag().to_string(),
                                module_path: module_parts.to_vec(),
                                container: Some(type_name.clone()),
                            },
                        });
                    }
                }
            }
            syn::Item::Mod(item_mod) => {
                if !visibility_allows(&item_mod.vis, include_private) {
                    continue;
                }
                if include_modules {
                    if let Some((docs, module_line)) = doc_from_attrs_with_line(&item_mod.attrs) {
                        let mut nested_parts = module_parts.to_vec();
                        nested_parts.push(item_mod.ident.to_string());
                        let mut nested_module_path = module_path.to_path_buf();
                        nested_module_path.push(item_mod.ident.to_string());
                        let qualified = nested_parts.join("::");
                        outputs.push(AdapterOutput {
                            output_rel_path: module_output_path(&nested_module_path),
                            frontmatter: build_frontmatter(
                                &qualified,
                                DocKind::Module,
                                Some(docs.as_str()),
                            ),
                            body_md: render_body(
                                &qualified,
                                &format!("mod {}", qualified),
                                Some(docs.as_str()),
                                repo_rel,
                                repo_url,
                                module_line,
                                DocKind::Module,
                                None,
                                "rust",
                            ),
                            source: SourceMeta {
                                file: repo_rel.to_path_buf(),
                                line: module_line,
                                repo_url: repo_url.map(|s| s.to_string()),
                            },
                            meta: DocMeta {
                                language: "rust".to_string(),
                                kind: DocKind::Module.tag().to_string(),
                                module_path: nested_parts.clone(),
                                container: None,
                            },
                        });
                        *module_docs_generated += 1;
                    }
                }

                if let Some((_, items)) = item_mod.content {
                    let mut nested_parts = module_parts.to_vec();
                    nested_parts.push(item_mod.ident.to_string());
                    let mut nested_module_path = module_path.to_path_buf();
                    nested_module_path.push(item_mod.ident.to_string());
                    process_items(
                        items,
                        &nested_parts,
                        &nested_module_path,
                        repo_rel,
                        repo_url,
                        include_private,
                        include_undocumented,
                        include_modules,
                        outputs,
                        module_docs_generated,
                        src,
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;
    use tempfile::tempdir;

    #[test]
    fn extracts_function_docs_with_line_numbers() {
        let dir = tempdir().unwrap();
        let src = r#"//! Module docs

/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let file_path = dir.path().join("lib.rs");
        fs::write(&file_path, src).unwrap();

        let adapter = RustDocAdapter::new();
        let mut opts = std::collections::HashMap::new();
        opts.insert("include_modules".to_string(), Value::Bool(true));
        let outputs = adapter
            .extract(dir.path(), None, &AdapterOptions::from_map(opts))
            .unwrap();

        assert!(outputs.len() >= 2);
        let func = outputs
            .iter()
            .find(|o| o.frontmatter.title == "add")
            .expect("function doc missing");
        assert_eq!(func.output_rel_path, PathBuf::from("add.md"));
        assert_eq!(func.frontmatter.note_type.as_deref(), Some("doc"));
        assert_eq!(func.frontmatter.slug.as_deref(), Some("add"));
        assert_eq!(func.source.file, PathBuf::from("lib.rs"));
        assert!(func.source.line.is_some());
        assert!(func.body_md.contains("Source"));
        assert!(func.body_md.contains("Adds two numbers."));

        let module = outputs
            .iter()
            .find(|o| o.output_rel_path == PathBuf::from("module.md"))
            .expect("module doc missing");
        assert_eq!(module.frontmatter.title, "crate");
    }

    #[test]
    fn extracts_inline_modules_and_methods() {
        let dir = tempdir().unwrap();
        let src = r#"//! Root docs

pub mod inner {
    //! Inner docs

    /// Adds two numbers.
    ///     - keeps indentation
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    pub struct Widget;

    impl Widget {
        /// Run the widget.
        pub fn run(&self) -> i32 {
            1
        }
    }
}
"#;
        let file_path = dir.path().join("lib.rs");
        fs::write(&file_path, src).unwrap();

        let adapter = RustDocAdapter::new();
        let mut opts = std::collections::HashMap::new();
        opts.insert("include_modules".to_string(), Value::Bool(true));
        let outputs = adapter
            .extract(
                dir.path(),
                Some("https://github.com/example/repo/blob/dev"),
                &AdapterOptions::from_map(opts),
            )
            .unwrap();

        let add = outputs
            .iter()
            .find(|o| o.output_rel_path == PathBuf::from("inner/add.md"))
            .expect("nested function doc missing");
        assert!(add.body_md.contains("- keeps indentation"));

        let method = outputs
            .iter()
            .find(|o| o.frontmatter.title == "inner::Widget::run")
            .expect("method doc missing");
        assert!(method.body_md.contains("pub fn run(&self)"));

        let inner_mod = outputs
            .iter()
            .find(|o| o.output_rel_path == PathBuf::from("inner/module.md"))
            .expect("inner module doc missing");
        assert!(inner_mod.body_md.contains("Inner docs"));

        let source = outputs
            .iter()
            .find(|o| o.frontmatter.title == "inner::Widget::run")
            .unwrap();
        assert!(source
            .body_md
            .contains("github.com/example/repo/blob/dev/lib.rs"));
    }
}
