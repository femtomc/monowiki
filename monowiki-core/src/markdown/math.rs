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
        // First pass: convert events to static and find math delimiters
        let mut static_events: Vec<Event<'static>> = Vec::new();
        let mut open: Option<OpenMath> = None;
        let mut in_code_block = false;
        let mut in_inline_code = false;

        for event in events {
            let owned = self.event_into_static(event);

            // Track code blocks and inline code to skip math processing
            match &owned {
                Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
                Event::End(TagEnd::CodeBlock) => in_code_block = false,
                Event::Code(_) => in_inline_code = true,
                _ => in_inline_code = false,
            }

            // Skip math processing inside code blocks or inline code
            if in_code_block || in_inline_code {
                static_events.push(owned);
                continue;
            }

            if let Some(mut state) = open.take() {
                // We're in an open math delimiter, look for the closer
                match owned {
                    Event::Text(text) => {
                        let text_str = text.into_string();
                        if let Some(close_idx) = text_str.find(state.close) {
                            // Found the closer!
                            state.content.push_str(&text_str[..close_idx]);
                            static_events.push(match state.kind {
                                DelimKind::Display => Event::DisplayMath(CowStr::Boxed(
                                    state.content.into_boxed_str(),
                                )),
                                DelimKind::Inline => Event::InlineMath(CowStr::Boxed(
                                    state.content.into_boxed_str(),
                                )),
                            });

                            // Process any remaining text after the closer
                            let remaining = text_str[close_idx + state.close.len()..].to_string();
                            if !remaining.is_empty() {
                                self.process_text_for_math(remaining, &mut static_events, &mut open);
                            }
                        } else {
                            // Closer not found, accumulate content
                            state.content.push_str(&text_str);
                            open = Some(state);
                        }
                    }
                    Event::SoftBreak => {
                        state.content.push('\n');
                        open = Some(state);
                    }
                    Event::HardBreak => {
                        state.content.push_str("\n\n");
                        open = Some(state);
                    }
                    other => {
                        // Unexpected event while in math mode - emit accumulated content as literal and the event
                        static_events.push(Event::Text(CowStr::Boxed(
                            format!("{}{}", state.open, state.content).into_boxed_str(),
                        )));
                        static_events.push(other);
                    }
                }
            } else {
                // Not in math mode
                match owned {
                    Event::Text(text) => {
                        self.process_text_for_math(text.into_string(), &mut static_events, &mut open);
                    }
                    other => static_events.push(other),
                }
            }
        }

        // If we still have an open math delimiter at the end, emit it as literal text
        if let Some(state) = open {
            static_events.push(Event::Text(CowStr::Boxed(
                format!("{}{}", state.open, state.content).into_boxed_str(),
            )));
        }

        static_events
    }

    /// Second-pass transform to unwrap paragraphs containing display math
    /// This must run AFTER math rendering (which converts DisplayMath to Html)
    pub fn unwrap_display_math_paragraphs(&self, events: Vec<Event<'static>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < events.len() {
            match &events[i] {
                Event::Start(Tag::Paragraph) => {
                    i += 1;
                    let mut paragraph: Vec<Event<'static>> = Vec::new();
                    while i < events.len() {
                        if matches!(events[i], Event::End(TagEnd::Paragraph)) {
                            break;
                        }
                        paragraph.push(events[i].clone());
                        i += 1;
                    }

                    // If the paragraph contains rendered display math (HTML), unwrap it
                    if self.paragraph_has_rendered_display_math(&paragraph) {
                        let mut buffer = Vec::new();
                        for ev in paragraph {
                            // Check if this is rendered display math HTML
                            let is_display_math = match &ev {
                                Event::Html(html) => html.contains("typst-display"),
                                Event::InlineHtml(html) => html.contains("typst-display"),
                                _ => false,
                            };

                            if is_display_math {
                                if !buffer.is_empty() {
                                    result.push(Event::Start(Tag::Paragraph));
                                    result.append(&mut buffer);
                                    result.push(Event::End(TagEnd::Paragraph));
                                }
                                result.push(ev);
                            } else {
                                buffer.push(ev);
                            }
                        }
                        if !buffer.is_empty() {
                            result.push(Event::Start(Tag::Paragraph));
                            result.append(&mut buffer);
                            result.push(Event::End(TagEnd::Paragraph));
                        }
                    } else {
                        result.push(Event::Start(Tag::Paragraph));
                        result.extend(paragraph);
                        if i < events.len() {
                            result.push(events[i].clone()); // End paragraph
                        }
                    }

                    // Skip the paragraph end (if present)
                    if i < events.len() {
                        i += 1;
                    }
                }
                _ => {
                    result.push(events[i].clone());
                    i += 1;
                }
            }
        }

        result
    }

    fn paragraph_has_rendered_display_math(&self, events: &[Event<'static>]) -> bool {
        events.iter().any(|event| {
            matches!(event, Event::Html(html) if html.contains("typst-display"))
                || matches!(event, Event::InlineHtml(html) if html.contains("typst-display"))
        })
    }

    fn process_text_for_math(
        &self,
        mut text: String,
        out: &mut Vec<Event<'static>>,
        open: &mut Option<OpenMath>,
    ) {
        while !text.is_empty() {
            if let Some((kind, start_idx, open_pat, close_pat)) = find_next_delimiter(&text) {
                // Emit any text before the delimiter
                if start_idx > 0 {
                    out.push(Event::Text(CowStr::Boxed(
                        text[..start_idx].to_string().into_boxed_str(),
                    )));
                }

                let after_start = &text[start_idx + open_pat.len()..];
                if let Some(end_idx) = after_start.find(close_pat) {
                    // Found the closer in the same text - emit math event
                    let math_content = &after_start[..end_idx];
                    out.push(match kind {
                        DelimKind::Display => Event::DisplayMath(CowStr::Boxed(
                            math_content.to_string().into_boxed_str(),
                        )),
                        DelimKind::Inline => Event::InlineMath(CowStr::Boxed(
                            math_content.to_string().into_boxed_str(),
                        )),
                    });
                    text = after_start[end_idx + close_pat.len()..].to_string();
                } else {
                    // No closer found in this text - set open state and return
                    *open = Some(OpenMath {
                        kind,
                        close: close_pat,
                        open: open_pat,
                        content: after_start.to_string(),
                    });
                    return;
                }
            } else {
                // No delimiter found - emit remaining text
                out.push(Event::Text(CowStr::Boxed(text.into_boxed_str())));
                break;
            }
        }
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

#[derive(Clone, Copy)]
enum DelimKind {
    Display,
    Inline,
}

struct OpenMath {
    kind: DelimKind,
    close: &'static str,
    open: &'static str,
    content: String,
}

fn find_next_delimiter(input: &str) -> Option<(DelimKind, usize, &'static str, &'static str)> {
    // Return the earliest occurrence among supported delimiters
    // Note: Check $$ before $ to avoid matching the first $ of $$
    let mut best: Option<(DelimKind, usize, &'static str, &'static str)> = None;

    // Check $$ first
    if let Some(idx) = input.find("$$") {
        best = Some((DelimKind::Display, idx, "$$", "$$"));
    }

    // Check other delimiters, preferring earlier matches
    let other_patterns = [
        (DelimKind::Display, "\\[", "\\]"),
        (DelimKind::Inline, "\\(", "\\)"),
        (DelimKind::Inline, "$", "$"),
    ];

    for (kind, open, close) in other_patterns {
        if let Some(idx) = input.find(open) {
            // For single $, make sure it's not part of $$
            if open == "$" {
                if idx > 0 && input.as_bytes().get(idx - 1) == Some(&b'$') {
                    continue; // This $ is the second $ of $$
                }
                if input.as_bytes().get(idx + 1) == Some(&b'$') {
                    continue; // This $ is the first $ of $$
                }
            }

            let candidate = (kind, idx, open, close);
            best = match best {
                Some(existing) if existing.1 < idx => Some(existing),
                Some(existing) if existing.1 == idx && existing.2.len() >= open.len() => {
                    Some(existing) // Prefer longer delimiter
                }
                _ => Some(candidate),
            };
        }
    }

    best
}

impl Default for MathTransformer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Event, Parser, Options};

    #[test]
    fn test_multiline_dollar_math() {
        // $$ works well for display math across lines
        let markdown = "$$\na + b\n$$";
        let options = Options::empty();
        let parser = Parser::new_ext(markdown, options);
        let events: Vec<Event> = parser.collect();

        let transformer = MathTransformer::new();
        let transformed = transformer.transform(events);

        let has_display_math = transformed
            .iter()
            .any(|e| matches!(e, Event::DisplayMath(_)));
        assert!(has_display_math, "Should contain DisplayMath event");
    }

    #[test]
    fn test_inline_dollar_math() {
        let markdown = "Test $x + y$ here";
        let options = Options::empty();
        let parser = Parser::new_ext(markdown, options);
        let events: Vec<Event> = parser.collect();

        let transformer = MathTransformer::new();
        let transformed = transformer.transform(events);

        let has_inline_math = transformed
            .iter()
            .any(|e| matches!(e, Event::InlineMath(_)));
        assert!(has_inline_math, "Should contain InlineMath event");
    }
}
