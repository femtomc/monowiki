---
title: builder::SiteBuilder::build
description: null
summary: Build the entire site
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:builder::SiteBuilder
draft: false
updated: null
slug: builder-sitebuilder-build
permalink: null
aliases:
- builder::SiteBuilder::build
typst_preamble: null
bibliography: []
target_slug: null
target_anchor: null
git_ref: null
quote: null
author: null
status: null
parent_id: null
---

# builder::SiteBuilder::build

**Kind:** Method

**Source:** [monowiki-core/src/builder.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/builder.rs#L45)

```rust
pub fn build(&self) -> Result<SiteIndex, BuildError>
```

Build the entire site

## Reference source: [monowiki-core/src/builder.rs L45–L168](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/builder.rs#L45)

```rust
    /// Build the entire site
    pub fn build(&self) -> Result<SiteIndex, BuildError> {
        // Create output directory
        fs::create_dir_all(self.config.output_dir())?;

        // Discover all markdown files
        let markdown_files = self.discover_markdown_files()?;

        tracing::info!("Found {} markdown files", markdown_files.len());

        let mut bibliography_store = BibliographyStore::new();
        bibliography_store.preload_paths(&self.config.bibliography_paths());

        // Parse all notes (first pass - without link resolution)
        let mut notes = Vec::new();
        let mut slug_map: HashMap<String, String> = HashMap::new();
        let base_url = self.config.normalized_base_url();
        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        for file_path in &markdown_files {
            match self.parse_note(file_path) {
                Ok(note) => {
                    // Check for duplicate slugs
                    if slug_map.contains_key(&note.slug) {
                        tracing::warn!("Duplicate slug: {}", note.slug);
                        return Err(BuildError::DuplicateSlug(note.slug.clone()));
                    }

                    let href = format!("{}{}", base_url, note.output_rel_path());
                    slug_map.insert(note.slug.clone(), href.clone());
                    // Aliases also resolve to the same target
                    for alias in &note.aliases {
                        let alias_slug = slugify(alias);
                        if let Some(existing) = slug_map.get(&alias_slug) {
                            // Only flag if the alias would point somewhere else
                            if existing != &href {
                                diagnostics.push(Diagnostic {
                                    code: "alias.duplicate".to_string(),
                                    message: format!(
                                        "Alias '{}' on '{}' conflicts with an existing target",
                                        alias, note.slug
                                    ),
                                    severity: DiagnosticSeverity::Warning,
                                    note_slug: Some(note.slug.clone()),
                                    source_path: note.source_path.clone(),
                                    context: Some(alias_slug.clone()),
                                    anchor: None,
                                });
                            }
                        } else {
                            slug_map.insert(alias_slug, href.clone());
                        }
                    }
                    notes.push(note);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {:?}: {}", file_path, e);
                    // Continue with other files
                }
            }
        }

        // Second pass - render markdown with link resolution
        for (idx, note) in notes.iter_mut().enumerate() {
            let markdown = match fs::read_to_string(&markdown_files[idx]) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to read {:?}: {}", markdown_files[idx], e);
                    continue;
                }
            };
            let (frontmatter, body) = parse_frontmatter(&markdown)?;

            let bibliography_paths = self.bibliography_paths(&frontmatter);
            let bibliography = bibliography_store.collect(&bibliography_paths);
            let citation_ctx = if bibliography.is_empty() {
                None
            } else {
                Some(CitationContext {
                    bibliography: &bibliography,
                })
            };

            let (html, outgoing_links, toc_html, mut note_diags) = self.processor.convert(
                &body,
                &slug_map,
                &base_url,
                citation_ctx.as_ref(),
                Some(&note.slug),
                note.source_path.as_deref(),
            );
            note.content_html = html;
            note.outgoing_links = outgoing_links;
            note.toc_html = toc_html;
            note.raw_body = Some(body);
            diagnostics.append(&mut note_diags);
        }

        // Build link graph
        let mut graph = LinkGraph::new();
        for note in &notes {
            if note.note_type == NoteType::Comment {
                continue;
            }
            for target in &note.outgoing_links {
                graph.add_link(&note.slug, target);
            }
        }

        // Carry over bibliography load diagnostics
        diagnostics.extend(bibliography_store.take_diagnostics());

        tracing::info!("Built site index with {} notes", notes.len());

        // Collect comments and resolve anchors
        let comments = collect_comments(&notes);

        Ok(SiteIndex {
            notes,
            graph,
            diagnostics,
            comments,
        })
    }
```
