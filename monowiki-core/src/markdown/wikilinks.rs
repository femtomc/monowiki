//! Wikilink transformation for [[target]] and [[target|text]] syntax.

use crate::slug::slugify;
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::collections::HashMap;

/// Transformer for wikilink syntax
pub struct WikilinkTransformer<'a> {
    slug_map: &'a HashMap<String, String>,
    base_url: String,
}

impl<'a> WikilinkTransformer<'a> {
    pub fn new(slug_map: &'a HashMap<String, String>, base_url: &str) -> Self {
        Self {
            slug_map,
            base_url: crate::config::normalize_base_url(base_url),
        }
    }

    /// Transform events, converting [[wikilinks]] to HTML links
    ///
    /// Returns (transformed_events, outgoing_links)
    pub fn transform(&self, events: Vec<Event<'_>>) -> (Vec<Event<'static>>, Vec<String>) {
        let mut result = Vec::new();
        let mut outgoing_links = Vec::new();
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
                    let (transformed, links) = self.process_wikilinks(&merged_text);
                    result.extend(transformed);
                    outgoing_links.extend(links);
                } else {
                    result.push(Event::Text(CowStr::Boxed(merged_text.into_boxed_str())));
                }
            } else {
                result.push(events[i].clone().into_static());
                i += 1;
            }
        }

        (result, outgoing_links)
    }

    fn process_wikilinks(&self, text: &str) -> (Vec<Event<'static>>, Vec<String>) {
        let mut events = Vec::new();
        let mut links = Vec::new();
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
                let (link_event, target_slug) = self.create_link(wikilink);
                events.extend(link_event);

                if let Some(slug) = target_slug {
                    links.push(slug);
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

        (events, links)
    }

    fn create_link(&self, wikilink: &str) -> (Vec<Event<'static>>, Option<String>) {
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
        (events, outgoing)
    }
}

/// Helper to convert Event<'a> to Event<'static>
trait IntoStatic {
    fn into_static(self) -> Event<'static>;
}

impl<'a> IntoStatic for Event<'a> {
    fn into_static(self) -> Event<'static> {
        match self {
            Event::Start(tag) => Event::Start(tag.into_static()),
            Event::End(tag) => Event::End(tag),
            Event::Text(text) => Event::Text(CowStr::Boxed(text.to_string().into_boxed_str())),
            Event::Code(code) => Event::Code(CowStr::Boxed(code.to_string().into_boxed_str())),
            Event::Html(html) => Event::Html(CowStr::Boxed(html.to_string().into_boxed_str())),
            Event::InlineHtml(html) => {
                Event::InlineHtml(CowStr::Boxed(html.to_string().into_boxed_str()))
            }
            Event::FootnoteReference(r) => {
                Event::FootnoteReference(CowStr::Boxed(r.to_string().into_boxed_str()))
            }
            Event::SoftBreak => Event::SoftBreak,
            Event::HardBreak => Event::HardBreak,
            Event::Rule => Event::Rule,
            Event::TaskListMarker(checked) => Event::TaskListMarker(checked),
            Event::InlineMath(math) => {
                Event::InlineMath(CowStr::Boxed(math.to_string().into_boxed_str()))
            }
            Event::DisplayMath(math) => {
                Event::DisplayMath(CowStr::Boxed(math.to_string().into_boxed_str()))
            }
        }
    }
}

trait TagIntoStatic {
    fn into_static(self) -> Tag<'static>;
}

impl<'a> TagIntoStatic for Tag<'a> {
    fn into_static(self) -> Tag<'static> {
        match self {
            Tag::Paragraph => Tag::Paragraph,
            Tag::Heading {
                level,
                id,
                classes,
                attrs,
            } => Tag::Heading {
                level,
                id: id.map(|s| CowStr::Boxed(s.to_string().into_boxed_str())),
                classes: classes
                    .into_iter()
                    .map(|s| CowStr::Boxed(s.to_string().into_boxed_str()))
                    .collect(),
                attrs: attrs
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            CowStr::Boxed(k.to_string().into_boxed_str()),
                            v.map(|s| CowStr::Boxed(s.to_string().into_boxed_str())),
                        )
                    })
                    .collect(),
            },
            Tag::BlockQuote(kind) => Tag::BlockQuote(kind),
            Tag::CodeBlock(kind) => Tag::CodeBlock(match kind {
                pulldown_cmark::CodeBlockKind::Indented => pulldown_cmark::CodeBlockKind::Indented,
                pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                    pulldown_cmark::CodeBlockKind::Fenced(CowStr::Boxed(
                        lang.to_string().into_boxed_str(),
                    ))
                }
            }),
            Tag::HtmlBlock => Tag::HtmlBlock,
            Tag::List(num) => Tag::List(num),
            Tag::Item => Tag::Item,
            Tag::FootnoteDefinition(label) => {
                Tag::FootnoteDefinition(CowStr::Boxed(label.to_string().into_boxed_str()))
            }
            Tag::Table(alignments) => Tag::Table(alignments),
            Tag::TableHead => Tag::TableHead,
            Tag::TableRow => Tag::TableRow,
            Tag::TableCell => Tag::TableCell,
            Tag::Emphasis => Tag::Emphasis,
            Tag::Strong => Tag::Strong,
            Tag::Strikethrough => Tag::Strikethrough,
            Tag::Superscript => Tag::Superscript,
            Tag::Subscript => Tag::Subscript,
            Tag::DefinitionList => Tag::DefinitionList,
            Tag::DefinitionListTitle => Tag::DefinitionListTitle,
            Tag::DefinitionListDefinition => Tag::DefinitionListDefinition,
            Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            } => Tag::Link {
                link_type,
                dest_url: CowStr::Boxed(dest_url.to_string().into_boxed_str()),
                title: CowStr::Boxed(title.to_string().into_boxed_str()),
                id: CowStr::Boxed(id.to_string().into_boxed_str()),
            },
            Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            } => Tag::Image {
                link_type,
                dest_url: CowStr::Boxed(dest_url.to_string().into_boxed_str()),
                title: CowStr::Boxed(title.to_string().into_boxed_str()),
                id: CowStr::Boxed(id.to_string().into_boxed_str()),
            },
            Tag::MetadataBlock(kind) => Tag::MetadataBlock(kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_wikilink() {
        let slug_map =
            HashMap::from([("rust-safety".to_string(), "/rust-safety.html".to_string())]);

        let transformer = WikilinkTransformer::new(&slug_map, "/");
        let events = vec![Event::Text(CowStr::Borrowed("Check out [[Rust Safety]]"))];

        let (_result, links) = transformer.transform(events);

        assert_eq!(links, vec!["rust-safety"]);
        // Should contain link events
        assert!(_result
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Link { .. }))));
    }

    #[test]
    fn test_wikilink_with_display_text() {
        let slug_map = HashMap::new();
        let transformer = WikilinkTransformer::new(&slug_map, "/");

        let events = vec![Event::Text(CowStr::Borrowed(
            "See [[rust-safety|this guide]]",
        ))];
        let (_result, links) = transformer.transform(events);

        assert_eq!(links, vec!["rust-safety"]);
    }

    #[test]
    fn test_multiple_wikilinks() {
        let slug_map = HashMap::new();
        let transformer = WikilinkTransformer::new(&slug_map, "/");

        let events = vec![Event::Text(CowStr::Borrowed(
            "Check [[Page One]] and [[Page Two]]",
        ))];
        let (_, links) = transformer.transform(events);

        assert_eq!(links.len(), 2);
        assert!(links.contains(&"page-one".to_string()));
        assert!(links.contains(&"page-two".to_string()));
    }

    #[test]
    fn test_wikilink_with_fragment() {
        let slug_map =
            HashMap::from([("rust-safety".to_string(), "/rust-safety.html".to_string())]);
        let transformer = WikilinkTransformer::new(&slug_map, "/");

        let events = vec![Event::Text(CowStr::Borrowed(
            "See [[Rust Safety#Memory Model]]",
        ))];
        let (result, links) = transformer.transform(events);

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
}
