//! Wikilink transformation for [[target]] and [[target|text]] syntax.

use crate::models::{Diagnostic, DiagnosticSeverity};
use crate::slug::slugify;
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::collections::HashMap;

/// Transformer for wikilink syntax
pub struct WikilinkTransformer<'a> {
    slug_map: &'a HashMap<String, String>,
    base_url: String,
    note_slug: Option<String>,
    source_path: Option<String>,
}

impl<'a> WikilinkTransformer<'a> {
    pub fn new(
        slug_map: &'a HashMap<String, String>,
        base_url: &str,
        note_slug: Option<String>,
        source_path: Option<String>,
    ) -> Self {
        Self {
            slug_map,
            base_url: crate::config::normalize_base_url(base_url),
            note_slug,
            source_path,
        }
    }

    /// Transform events, converting [[wikilinks]] to HTML links
    ///
    /// Returns (transformed_events, outgoing_links)
    pub fn transform(
        &self,
        events: Vec<Event<'_>>,
    ) -> (Vec<Event<'static>>, Vec<String>, Vec<Diagnostic>) {
        let mut result = Vec::new();
        let mut outgoing_links = Vec::new();
        let mut diagnostics = Vec::new();
        let mut i = 0;
        let mut in_code_block = false;

        while i < events.len() {
            // Track code block context
            match &events[i] {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    result.push(events[i].clone().into_static());
                    i += 1;
                    continue;
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    result.push(events[i].clone().into_static());
                    i += 1;
                    continue;
                }
                _ => {}
            }

            // Skip wikilink processing inside code blocks
            if in_code_block {
                result.push(events[i].clone().into_static());
                i += 1;
                continue;
            }

            if let Event::Text(_) = &events[i] {
                // Collect all consecutive Text events and merge them
                let mut merged_text = String::new();

                while i < events.len() {
                    if let Event::Text(text) = &events[i] {
                        merged_text.push_str(text.as_ref());
                        i += 1;
                    } else {
                        break;
                    }
                }

                // Check if merged text contains wikilink syntax
                if merged_text.contains("[[") && merged_text.contains("]]") {
                    let (transformed, links, mut diags) = self.process_wikilinks(&merged_text);
                    result.extend(transformed);
                    outgoing_links.extend(links);
                    diagnostics.append(&mut diags);
                } else {
                    result.push(Event::Text(CowStr::Boxed(merged_text.into_boxed_str())));
                }
            } else {
                result.push(events[i].clone().into_static());
                i += 1;
            }
        }

        (result, outgoing_links, diagnostics)
    }

    fn process_wikilinks(&self, text: &str) -> (Vec<Event<'static>>, Vec<String>, Vec<Diagnostic>) {
        let mut events = Vec::new();
        let mut links = Vec::new();
        let mut diagnostics = Vec::new();
        let mut remaining = text;

        while let Some(start) = remaining.find("[[") {
            // Add text before the wikilink
            if start > 0 {
                events.push(Event::Text(CowStr::Boxed(
                    remaining[..start].to_string().into_boxed_str(),
                )));
            }

            // Find the closing ]]
            if let Some(end) = remaining[start..].find("]]") {
                let wikilink = &remaining[start + 2..start + end];
                let (link_event, target_slug, diag) = self.create_link(wikilink);
                events.extend(link_event);

                if let Some(slug) = target_slug {
                    links.push(slug);
                }
                if let Some(diag) = diag {
                    diagnostics.push(diag);
                }

                remaining = &remaining[start + end + 2..];
            } else {
                // No closing ]], treat as literal text
                events.push(Event::Text(CowStr::Boxed(
                    remaining.to_string().into_boxed_str(),
                )));
                break;
            }
        }

        // Add any remaining text
        if !remaining.is_empty() {
            events.push(Event::Text(CowStr::Boxed(
                remaining.to_string().into_boxed_str(),
            )));
        }

        (events, links, diagnostics)
    }

    fn create_link(
        &self,
        wikilink: &str,
    ) -> (Vec<Event<'static>>, Option<String>, Option<Diagnostic>) {
        // Parse [[target|display text]] or [[target]]
        let (target, display) = if let Some(pipe_pos) = wikilink.find('|') {
            let target = wikilink[..pipe_pos].trim();
            let display = wikilink[pipe_pos + 1..].trim();
            (target, Some(display))
        } else {
            (wikilink.trim(), None)
        };

        let (target_base, fragment_raw) = if let Some((base, frag)) = target.split_once('#') {
            (base.trim(), Some(frag.trim()))
        } else {
            (target, None)
        };

        let slug = slugify(target_base);
        let fragment = fragment_raw.filter(|f| !f.is_empty()).map(slugify);

        let display_text = display.unwrap_or(target);

        // Check if target exists in slug map
        let href = if let Some(dest) = self.slug_map.get(&slug) {
            if let Some(frag) = &fragment {
                format!("{dest}#{frag}")
            } else {
                dest.clone()
            }
        } else {
            // Target doesn't exist, but still create a link
            // Use base_url to avoid breaking subpath deployments
            let base = format!("{}{}.html", self.base_url, slug);
            if let Some(frag) = &fragment {
                format!("{base}#{frag}")
            } else {
                base
            }
        };

        let mut events = Vec::new();
        let mut diagnostic = None;

        // Create link: <a href="...">
        events.push(Event::Start(Tag::Link {
            link_type: pulldown_cmark::LinkType::Inline,
            dest_url: CowStr::Boxed(href.into_boxed_str()),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }));

        // Link text
        events.push(Event::Text(CowStr::Boxed(
            display_text.to_string().into_boxed_str(),
        )));

        // Close link: </a>
        events.push(Event::End(TagEnd::Link));

        let outgoing = if slug.is_empty() { None } else { Some(slug) };
        if outgoing.is_some() && !self.slug_map.contains_key(outgoing.as_ref().unwrap()) {
            diagnostic = Some(Diagnostic {
                code: "link.unresolved".to_string(),
                message: format!("Unresolved wikilink target '{}'", target),
                severity: DiagnosticSeverity::Warning,
                note_slug: self.note_slug.clone(),
                source_path: self.source_path.clone(),
                context: Some(target.to_string()),
                anchor: None,
            });
        }

        (events, outgoing, diagnostic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_wikilink() {
        let slug_map =
            HashMap::from([("rust-safety".to_string(), "/rust-safety.html".to_string())]);

        let transformer = WikilinkTransformer::new(&slug_map, "/", None, None);
        let events = vec![Event::Text(CowStr::Borrowed("Check out [[Rust Safety]]"))];

        let (_result, links, _diags) = transformer.transform(events);

        assert_eq!(links, vec!["rust-safety"]);
        // Should contain link events
        assert!(_result
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Link { .. }))));
    }

    #[test]
    fn test_wikilink_with_display_text() {
        let slug_map = HashMap::new();
        let transformer = WikilinkTransformer::new(&slug_map, "/", None, None);

        let events = vec![Event::Text(CowStr::Borrowed(
            "See [[rust-safety|this guide]]",
        ))];
        let (_result, links, _diags) = transformer.transform(events);

        assert_eq!(links, vec!["rust-safety"]);
    }

    #[test]
    fn test_multiple_wikilinks() {
        let slug_map = HashMap::new();
        let transformer = WikilinkTransformer::new(&slug_map, "/", None, None);

        let events = vec![Event::Text(CowStr::Borrowed(
            "Check [[Page One]] and [[Page Two]]",
        ))];
        let (_, links, _diags) = transformer.transform(events);

        assert_eq!(links.len(), 2);
        assert!(links.contains(&"page-one".to_string()));
        assert!(links.contains(&"page-two".to_string()));
    }

    #[test]
    fn test_wikilink_with_fragment() {
        let slug_map =
            HashMap::from([("rust-safety".to_string(), "/rust-safety.html".to_string())]);
        let transformer = WikilinkTransformer::new(&slug_map, "/", None, None);

        let events = vec![Event::Text(CowStr::Borrowed(
            "See [[Rust Safety#Memory Model]]",
        ))];
        let (result, links, _diags) = transformer.transform(events);

        assert_eq!(links, vec!["rust-safety"]);

        let href = result.iter().find_map(|event| {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                Some(dest_url.as_ref().to_string())
            } else {
                None
            }
        });

        assert_eq!(
            href.as_deref(),
            Some("/rust-safety.html#memory-model"),
            "href should include fragment slug"
        );
    }

    #[test]
    fn test_collects_unresolved_wikilink_diagnostic() {
        let slug_map = HashMap::new();
        let transformer = WikilinkTransformer::new(
            &slug_map,
            "/",
            Some("note-a".to_string()),
            Some("path/to/note.md".to_string()),
        );

        let events = vec![Event::Text(CowStr::Borrowed("See [[Missing Note]]"))];
        let (_result, _links, diags) = transformer.transform(events);

        assert_eq!(
            diags.len(),
            1,
            "Should emit one diagnostic for unresolved link"
        );
        let diag = &diags[0];
        assert_eq!(diag.code, "link.unresolved");
        assert_eq!(diag.note_slug.as_deref(), Some("note-a"));
        assert_eq!(diag.source_path.as_deref(), Some("path/to/note.md"));
    }
}
