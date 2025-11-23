//! Inline citation handling and reference list rendering.

use crate::bibliography::Bibliography;
use hayagriva::{types::Person, Entry};
use once_cell::sync::Lazy;
use pulldown_cmark::{CowStr, Event};
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write;
use tracing::warn;

/// Context required to resolve citation keys.
pub struct CitationContext<'a> {
    pub bibliography: &'a Bibliography,
}

/// A single reference entry in the rendered bibliography.
#[derive(Debug, Clone)]
pub struct CitationRef {
    pub key: String,
    pub number: usize,
    pub entry: Option<Entry>,
}

/// Transform markdown events by replacing `[@key]` markers with inline citations.
pub struct CitationTransformer<'a> {
    ctx: &'a CitationContext<'a>,
    order: Vec<String>,
    index: HashMap<String, usize>,
}

impl<'a> CitationTransformer<'a> {
    pub fn new(ctx: &'a CitationContext<'a>) -> Self {
        Self {
            ctx,
            order: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn transform(
        mut self,
        events: Vec<Event<'static>>,
    ) -> (Vec<Event<'static>>, Vec<CitationRef>) {
        let mut out = Vec::with_capacity(events.len());

        for event in events {
            match event {
                Event::Text(text) => {
                    let mut last_end = 0;
                    let mut replaced = false;
                    for caps in CITE_RE.captures_iter(&text) {
                        if let Some(full) = caps.get(0) {
                            let start = full.start();
                            let end = full.end();
                            if start > last_end {
                                out.push(Event::Text(CowStr::Boxed(
                                    text[last_end..start].to_string().into_boxed_str(),
                                )));
                            }
                            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
                            let rendered = self.render_citation(inner);
                            out.push(Event::Html(CowStr::Boxed(rendered.into_boxed_str())));
                            last_end = end;
                            replaced = true;
                        }
                    }

                    if replaced && last_end < text.len() {
                        out.push(Event::Text(CowStr::Boxed(
                            text[last_end..].to_string().into_boxed_str(),
                        )));
                    }

                    if !replaced {
                        out.push(Event::Text(text));
                    }
                }
                other => out.push(other),
            }
        }

        let references = self
            .order
            .iter()
            .enumerate()
            .map(|(idx, key)| {
                let entry = self.ctx.bibliography.get(key).cloned();
                if entry.is_none() {
                    warn!("Missing bibliography entry for key '{}'", key);
                }
                CitationRef {
                    key: key.clone(),
                    number: idx + 1,
                    entry,
                }
            })
            .collect();

        (out, references)
    }

    fn render_citation(&mut self, inner: &str) -> String {
        let keys: Vec<String> = inner
            .split(|c| c == ';' || c == ',')
            .map(|s| s.trim().trim_start_matches('@'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if keys.is_empty() {
            return format!("[{}]", html_escape(inner));
        }

        let numbers: Vec<usize> = keys.iter().map(|k| self.register(k)).collect();
        let label = numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let first = numbers[0];
        let data_cites = keys.join(",");
        format!(
            "<sup class=\"citation\" id=\"cite-{first}\" data-cites=\"{data_cites}\"><a href=\"#ref-{first}\">[{label}]</a></sup>",
            first = first,
            data_cites = html_escape(&data_cites),
            label = label,
        )
    }

    fn register(&mut self, key: &str) -> usize {
        if let Some(num) = self.index.get(key) {
            *num
        } else {
            let num = self.order.len() + 1;
            self.order.push(key.to_string());
            self.index.insert(key.to_string(), num);
            num
        }
    }
}

/// Render the collected references as an HTML list.
pub fn render_references(references: &[CitationRef]) -> Option<String> {
    if references.is_empty() {
        return None;
    }

    let mut html = String::from(
        r#"<section class="references"><h3>References</h3><ol class="reference-list">"#,
    );
    for cite in references {
        html.push_str(&format!(r#"<li id="ref-{}">"#, cite.number));
        let body = cite
            .entry
            .as_ref()
            .map(|entry| format_entry(entry))
            .unwrap_or_else(|| format!("Missing entry: {}", html_escape(&cite.key)));
        html.push_str(&body);
        html.push_str(&format!(
            " <a class=\"ref-backlink\" href=\"#cite-{}\" aria-label=\"Back to citation\">&#8617;</a>",
            cite.number
        ));
        html.push_str("</li>");
    }
    html.push_str("</ol></section>");

    Some(html)
}

fn format_entry(entry: &Entry) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(authors) = entry.authors() {
        if !authors.is_empty() {
            parts.push(format_authors(authors));
        }
    } else if let Some(editors) = entry.editors() {
        if !editors.is_empty() {
            parts.push(format_authors(editors));
        }
    }

    if let Some(date) = entry.date() {
        parts.push(format!("({})", date));
    }

    if let Some(title) = entry.title() {
        parts.push(format!(
            r#"<span class="ref-title">{}</span>"#,
            html_escape(&title.to_string())
        ));
    }

    if let Some(parent) = entry.parents().first().and_then(|p| p.title()) {
        parts.push(format!(r#"<em>{}</em>"#, html_escape(&parent.to_string())));
    }

    if let Some(publisher) = entry.publisher() {
        if let Some(name) = publisher.name() {
            parts.push(html_escape(&name.to_string()));
        }
        if let Some(location) = publisher.location() {
            parts.push(html_escape(&location.to_string()));
        }
    }

    if let Some(url) = entry.url() {
        let escaped = html_escape(&url.to_string());
        parts.push(format!(r#"<a href="{0}">{0}</a>"#, escaped));
    } else if let Some(serial) = entry.serial_number() {
        if let Some(doi) = serial.0.get("doi") {
            let escaped = html_escape(doi);
            parts.push(format!(
                r#"doi: <a href="https://doi.org/{0}">{0}</a>"#,
                escaped
            ));
        }
    }

    if parts.is_empty() {
        html_escape(entry.key())
    } else {
        parts.join(". ")
    }
}

fn format_authors(authors: &[Person]) -> String {
    let names: Vec<String> = authors
        .iter()
        .map(|p| html_escape(&p.name_first(true, false)))
        .collect();

    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        2 => format!("{} & {}", names[0], names[1]),
        _ => {
            let mut out = names[..names.len() - 1].join(", ");
            let _ = write!(&mut out, ", & {}", names.last().unwrap());
            out
        }
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

static CITE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[@([^\]]+)\]").expect("valid citation regex"));
