//! Math delimiter transformer to normalize math blocks before rendering.

use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

/// Transformer to improve math rendering
pub struct MathTransformer;

impl MathTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Transform events to improve math rendering
    ///
    /// Detects $$ blocks and ensures they're in their own paragraphs
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < events.len() {
            match &events[i] {
                Event::Start(Tag::Paragraph) => {
                    // Look ahead to see if this paragraph contains display math
                    if self.paragraph_has_display_math(&events[i..]) {
                        // Skip the paragraph tags, just output the content
                        i += 1; // Skip Start(Paragraph)
                        while i < events.len() {
                            if matches!(events[i], Event::End(TagEnd::Paragraph)) {
                                i += 1; // Skip End(Paragraph)
                                break;
                            }
                            result.push(self.event_into_static(events[i].clone()));
                            i += 1;
                        }
                    } else {
                        result.push(self.event_into_static(events[i].clone()));
                        i += 1;
                    }
                }
                _ => {
                    result.push(self.event_into_static(events[i].clone()));
                    i += 1;
                }
            }
        }

        result
    }

    fn paragraph_has_display_math(&self, events: &[Event]) -> bool {
        for event in events {
            if let Event::Text(text) = event {
                let s = text.as_ref();
                if s.contains("$$") && (s.starts_with("$$") || s.ends_with("$$")) {
                    return true;
                }
            }
            if matches!(event, Event::End(TagEnd::Paragraph)) {
                break;
            }
        }
        false
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

    fn tag_into_static(&self, tag: Tag<'_>) -> Tag<'static> {
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

impl Default for MathTransformer {
    fn default() -> Self {
        Self::new()
    }
}
