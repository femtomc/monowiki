//! Sidenote transformation for [^sidenote: text] syntax.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

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
        const SIDENOTE_PREFIX: &str = "[^sidenote:";

        while let Some(start) = remaining.find(SIDENOTE_PREFIX) {
            // Add text before the sidenote
            if start > 0 {
                events.push(Event::Text(CowStr::Boxed(
                    remaining[..start].to_string().into_boxed_str(),
                )));
            }

            // Find the closing ]
            let search_start = start + SIDENOTE_PREFIX.len();
            if let Some(end) = find_closing_bracket(remaining, search_start) {
                let content = &remaining[search_start..end];

                // Increment counter
                let num = self.counter.get() + 1;
                self.counter.set(num);

                let ref_id = format!("sidenote-ref-{}", num);
                let note_id = format!("sidenote-{}", num);
                let rendered_content = self.render_sidenote_content(content.trim());

                // Create sidenote HTML
                let sidenote_html = format!(
                    "<sup class=\"sidenote-ref\" id=\"{ref_id}\">\
                        <a href=\"#{note_id}\" aria-label=\"Sidenote {num}\">{num}</a>\
                    </sup>\
                    <span class=\"sidenote\" id=\"{note_id}\" role=\"note\" aria-describedby=\"{ref_id}\">\
                        <span class=\"sidenote-number\">{num}</span>{content}\
                    </span>",
                    ref_id = ref_id,
                    note_id = note_id,
                    num = num,
                    content = rendered_content
                );

                events.push(Event::InlineHtml(CowStr::Boxed(
                    sidenote_html.into_boxed_str(),
                )));

                remaining = &remaining[end + 1..];
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

    fn render_sidenote_content(&self, content: &str) -> String {
        // Render a limited, inline-only subset of Markdown while escaping raw HTML.
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        opts.insert(Options::ENABLE_MATH);

        let parser = Parser::new_ext(content, opts);
        let mut rendered = String::new();

        for event in parser {
            match event {
                Event::Text(text) => rendered.push_str(&html_escape(text.as_ref())),
                Event::Code(code) => {
                    rendered.push_str("<code>");
                    rendered.push_str(&html_escape(code.as_ref()));
                    rendered.push_str("</code>");
                }
                Event::SoftBreak | Event::HardBreak => rendered.push_str("<br>"),
                Event::Start(Tag::Emphasis) => rendered.push_str("<em>"),
                Event::End(TagEnd::Emphasis) => rendered.push_str("</em>"),
                Event::Start(Tag::Strong) => rendered.push_str("<strong>"),
                Event::End(TagEnd::Strong) => rendered.push_str("</strong>"),
                Event::Start(Tag::Strikethrough) => rendered.push_str("<del>"),
                Event::End(TagEnd::Strikethrough) => rendered.push_str("</del>"),
                Event::Start(Tag::Link {
                    dest_url, title, ..
                }) => {
                    rendered.push_str("<a href=\"");
                    rendered.push_str(&html_escape(dest_url.as_ref()));
                    rendered.push('"');
                    if !title.is_empty() {
                        rendered.push_str(" title=\"");
                        rendered.push_str(&html_escape(title.as_ref()));
                        rendered.push('"');
                    }
                    rendered.push('>');
                }
                Event::End(TagEnd::Link) => rendered.push_str("</a>"),
                Event::InlineHtml(html) | Event::Html(html) => {
                    // Escape any raw HTML to avoid XSS in sidenotes.
                    rendered.push_str(&html_escape(html.as_ref()));
                }
                Event::InlineMath(math) => {
                    rendered.push_str("<span class=\"math-inline\">");
                    rendered.push_str(&html_escape(math.as_ref()));
                    rendered.push_str("</span>");
                }
                Event::DisplayMath(math) => {
                    rendered.push_str("<span class=\"math-display\">");
                    rendered.push_str(&html_escape(math.as_ref()));
                    rendered.push_str("</span>");
                }
                // Drop block-level wrappers to keep sidenotes inline-friendly.
                Event::Start(
                    Tag::Paragraph
                    | Tag::Heading { .. }
                    | Tag::List(_)
                    | Tag::Item
                    | Tag::DefinitionList
                    | Tag::DefinitionListTitle
                    | Tag::DefinitionListDefinition
                    | Tag::BlockQuote(_)
                    | Tag::CodeBlock(_)
                    | Tag::Table(_)
                    | Tag::TableHead
                    | Tag::TableRow
                    | Tag::TableCell,
                ) => {}
                Event::End(
                    TagEnd::Paragraph
                    | TagEnd::Heading(_)
                    | TagEnd::List(_)
                    | TagEnd::Item
                    | TagEnd::DefinitionList
                    | TagEnd::Table
                    | TagEnd::TableHead
                    | TagEnd::TableRow
                    | TagEnd::TableCell
                    | TagEnd::BlockQuote(_)
                    | TagEnd::CodeBlock,
                ) => {}
                Event::FootnoteReference(label) => {
                    rendered.push_str("<sup class=\"footnote-ref\">");
                    rendered.push_str(&html_escape(label.as_ref()));
                    rendered.push_str("</sup>");
                }
                Event::Rule | Event::TaskListMarker(_) => {}
                _ => {}
            }
        }

        rendered
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

fn find_closing_bracket(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut iter = text[start..].char_indices();

    while let Some((offset, ch)) = iter.next() {
        match ch {
            '\\' => {
                // Skip escaped characters
                iter.next();
            }
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    return Some(start + offset);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    None
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
        assert!(html_events[0].contains("sidenote-ref"));
        assert!(html_events[0].contains("sidenote-number"));
        assert!(html_events[0].contains("with a note"));
        assert!(html_events[0].contains("aria-describedby"));
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
        assert!(html_events[0].contains("sidenote-ref-1"));
        assert!(html_events[0].contains("sidenote-1"));
        assert!(html_events[1].contains("sidenote-ref-2"));
        assert!(html_events[1].contains("sidenote-2"));
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

    #[test]
    fn test_sidenote_with_brackets() {
        let transformer = SidenoteTransformer::new();
        let events = vec![Event::Text(CowStr::Borrowed(
            "Note with [^sidenote: link [text] inside] continues.",
        ))];

        let result = transformer.transform(events);
        let inline_html: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                Event::InlineHtml(html) => Some(html.as_ref()),
                _ => None,
            })
            .collect();

        assert_eq!(inline_html.len(), 1);
        assert!(inline_html[0].contains("link [text] inside"));
        assert!(result
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t.contains("continues."))));
    }

    #[test]
    fn test_sidenote_renders_markdown() {
        let transformer = SidenoteTransformer::new();
        let events = vec![Event::Text(CowStr::Borrowed(
            "Content [^sidenote: *em* and a [link](https://example.com)] here.",
        ))];

        let result = transformer.transform(events);
        let inline_html: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                Event::InlineHtml(html) => Some(html.as_ref()),
                _ => None,
            })
            .collect();

        assert_eq!(inline_html.len(), 1);
        let html = inline_html[0];
        assert!(html.contains("<em>em</em>"));
        assert!(html.contains(r#"href="https://example.com""#));
    }
}
