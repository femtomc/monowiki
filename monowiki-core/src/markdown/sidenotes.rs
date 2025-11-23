//! Sidenote transformation for [^sidenote: text] syntax.

use pulldown_cmark::{CowStr, Event};

/// Transformer for sidenote syntax
pub struct SidenoteTransformer {
    counter: std::cell::Cell<usize>,
}

impl SidenoteTransformer {
    pub fn new() -> Self {
        Self {
            counter: std::cell::Cell::new(0),
        }
    }

    /// Transform events, converting [^sidenote: text] to HTML spans
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();

        for event in events {
            if let Event::Text(text) = &event {
                let text_str = text.as_ref();

                // Check if this text contains sidenote syntax
                if text_str.contains("[^sidenote:") && text_str.contains("]") {
                    result.extend(self.process_sidenotes(text_str));
                } else {
                    result.push(Event::Text(CowStr::Boxed(
                        text_str.to_string().into_boxed_str(),
                    )));
                }
            } else {
                result.push(self.event_into_static(event));
            }
        }

        result
    }

    fn process_sidenotes(&self, text: &str) -> Vec<Event<'static>> {
        let mut events = Vec::new();
        let mut remaining = text;

        while let Some(start) = remaining.find("[^sidenote:") {
            // Add text before the sidenote
            if start > 0 {
                events.push(Event::Text(CowStr::Boxed(
                    remaining[..start].to_string().into_boxed_str(),
                )));
            }

            // Find the closing ]
            if let Some(end) = remaining[start..].find(']') {
                let content = &remaining[start + 11..start + end]; // Skip "[^sidenote:"

                // Increment counter
                let num = self.counter.get() + 1;
                self.counter.set(num);

                // Create sidenote HTML
                let sidenote_html = format!(
                    r#"<span class="sidenote"><span class="sidenote-number">{}</span>{}</span>"#,
                    num,
                    html_escape(content.trim())
                );

                events.push(Event::InlineHtml(CowStr::Boxed(
                    sidenote_html.into_boxed_str(),
                )));

                remaining = &remaining[start + end + 1..];
            } else {
                // No closing ], treat as literal text
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

        events
    }

    fn event_into_static(&self, event: Event<'_>) -> Event<'static> {
        match event {
            Event::Start(tag) => Event::Start(self.tag_into_static(tag)),
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

    fn tag_into_static(&self, tag: pulldown_cmark::Tag<'_>) -> pulldown_cmark::Tag<'static> {
        use pulldown_cmark::Tag;

        match tag {
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

impl Default for SidenoteTransformer {
    fn default() -> Self {
        Self::new()
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_sidenote() {
        let transformer = SidenoteTransformer::new();
        let events = vec![Event::Text(CowStr::Borrowed(
            "This is text [^sidenote: with a note] here.",
        ))];

        let result = transformer.transform(events);

        // Should contain inline HTML with sidenote
        let html_events: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                Event::InlineHtml(html) => Some(html.as_ref()),
                _ => None,
            })
            .collect();

        assert!(!html_events.is_empty());
        assert!(html_events[0].contains("sidenote"));
        assert!(html_events[0].contains("sidenote-number"));
        assert!(html_events[0].contains("with a note"));
    }

    #[test]
    fn test_multiple_sidenotes() {
        let transformer = SidenoteTransformer::new();
        let events = vec![Event::Text(CowStr::Borrowed(
            "First [^sidenote: note one] and second [^sidenote: note two].",
        ))];

        let result = transformer.transform(events);

        let html_events: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                Event::InlineHtml(html) => Some(html.as_ref()),
                _ => None,
            })
            .collect();

        assert_eq!(html_events.len(), 2);
        assert!(html_events[0].contains("sidenote-number\">1<"));
        assert!(html_events[1].contains("sidenote-number\">2<"));
    }

    #[test]
    fn test_html_escaping() {
        let transformer = SidenoteTransformer::new();
        let events = vec![Event::Text(CowStr::Borrowed(
            "Text [^sidenote: <script>alert('xss')</script>] here.",
        ))];

        let result = transformer.transform(events);

        let html_events: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                Event::InlineHtml(html) => Some(html.as_ref()),
                _ => None,
            })
            .collect();

        assert!(!html_events[0].contains("<script>"));
        assert!(html_events[0].contains("&lt;script&gt;"));
    }
}
