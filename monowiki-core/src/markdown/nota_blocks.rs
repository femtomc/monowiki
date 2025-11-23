//! Lightweight Nota-like block syntax for Markdown.
//!
//! Recognizes paragraphs that start with `@Definition[label]{Title}: ...`
//! and wraps their content in a styled block while preserving downstream
//! transformations (math, wikilinks, etc.).

use crate::slug::slugify;
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct NotaBlockTransformer;

impl NotaBlockTransformer {
    pub fn new() -> Self {
        Self
    }

    pub fn transform(&self, events: Vec<Event<'static>>) -> Vec<Event<'static>> {
        let mut out = Vec::new();
        let mut i = 0;

        while i < events.len() {
            match &events[i] {
                Event::Start(Tag::Paragraph) => {
                    let mut paragraph = Vec::new();
                    let start_idx = i;
                    i += 1;
                    while i < events.len() {
                        if matches!(events[i], Event::End(TagEnd::Paragraph)) {
                            break;
                        }
                        paragraph.push(events[i].clone());
                        i += 1;
                    }

                    let rewritten = self.rewrite_paragraph(&paragraph);
                    if let Some(mut block_events) = rewritten {
                        out.append(&mut block_events);
                    } else {
                        out.push(events[start_idx].clone());
                        out.append(&mut paragraph);
                        if i < events.len() {
                            out.push(events[i].clone()); // End paragraph
                        }
                    }

                    // Skip the paragraph end (if present)
                    if i < events.len() {
                        i += 1;
                    }
                }
                _ => {
                    out.push(events[i].clone());
                    i += 1;
                }
            }
        }

        out
    }

    fn rewrite_paragraph(&self, paragraph: &[Event<'static>]) -> Option<Vec<Event<'static>>> {
        let mut flat = String::new();
        for event in paragraph {
            match event {
                Event::Text(text) => flat.push_str(text),
                Event::SoftBreak | Event::HardBreak => flat.push('\n'),
                _ => break,
            }
        }

        let caps = block_regex().captures(&flat)?;
        let kind_raw = caps.name("kind")?.as_str();
        let kind = kind_raw.to_lowercase();
        let title = caps.name("title").map(|m| m.as_str().trim()).unwrap_or("");
        let label = caps
            .name("label")
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| slugify(title));
        let label = label
            .strip_prefix("label=")
            .map(|s| s.to_string())
            .unwrap_or(label);
        let consumed = caps
            .name("rest")
            .map(|m| m.start())
            .unwrap_or_else(|| caps.get(0).map(|m| m.end()).unwrap_or(0));

        // Build the body events by stripping the prefix text across the paragraph
        let mut body_events: Vec<Event<'static>> = Vec::new();
        let mut remaining = consumed;
        for event in paragraph {
            match event {
                Event::Text(text) => {
                    if remaining == 0 {
                        body_events.push(event.clone());
                        continue;
                    }
                    let len = text.len();
                    if remaining >= len {
                        remaining -= len;
                        continue;
                    }
                    let new_text = text[remaining..].to_string();
                    body_events.push(Event::Text(CowStr::Boxed(
                        new_text.into_boxed_str(),
                    )));
                    remaining = 0;
                }
                Event::SoftBreak => {
                    if remaining > 0 {
                        remaining = remaining.saturating_sub(1);
                        if remaining == 0 {
                            body_events.push(Event::SoftBreak);
                        }
                    } else {
                        body_events.push(Event::SoftBreak);
                    }
                }
                Event::HardBreak => {
                    if remaining > 0 {
                        remaining = remaining.saturating_sub(1);
                        if remaining == 0 {
                            body_events.push(Event::HardBreak);
                        }
                    } else {
                        body_events.push(Event::HardBreak);
                    }
                }
                _ => body_events.push(event.clone()),
            }
        }

        let mut out = Vec::new();
        out.push(Event::Html(CowStr::Boxed(
            render_open(&kind, &label, title).into_boxed_str(),
        )));
        out.push(Event::Start(Tag::Paragraph));
        out.extend(body_events);
        out.push(Event::End(TagEnd::Paragraph));
        out.push(Event::Html(CowStr::Boxed(render_close().into_boxed_str())));
        Some(out)
    }
}

fn block_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*@(?P<kind>[A-Za-z]+)(?:\[(?P<label>[^\]]+)\])?\{(?P<title>[^}]*)\}\s*:?\s*(?P<rest>.*)$",
        )
            .expect("valid regex")
    })
}

fn render_open(kind: &str, id: &str, title: &str) -> String {
    let kind_label = capitalize(kind);
    let mut html = format!(
        r#"<div class="nota-block nota-{}" id="{}"><div class="nota-block-heading"><span class="nota-kind">{}</span>"#,
        html_escape(kind),
        html_escape(id),
        html_escape(&kind_label)
    );
    if !title.is_empty() {
        html.push_str(&format!(
            r#"<span class="nota-title">{}</span>"#,
            html_escape(title)
        ));
    }
    html.push_str(r#"</div><div class="nota-block-body">"#);
    html
}

fn render_close() -> String {
    "</div></div>".to_string()
}

fn capitalize(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
