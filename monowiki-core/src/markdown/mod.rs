//! Markdown processing pipeline with custom extensions.

pub mod citations;
pub mod highlight;
pub mod math;
pub mod nota_blocks;
pub mod sidenotes;
pub mod typst_math;
pub mod wikilinks;

#[cfg(test)]
mod test_integration;

#[cfg(test)]
mod debug_events;

use crate::slug::slugify;
use citations::{render_references, CitationContext, CitationTransformer};
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use std::collections::HashMap;

pub use highlight::HighlightTransformer;
pub use math::MathTransformer;
pub use nota_blocks::NotaBlockTransformer;
pub use sidenotes::SidenoteTransformer;
use typst_math::MATH_RENDERER;
pub use wikilinks::WikilinkTransformer;

#[derive(Debug, Clone)]
struct TocItem {
    level: u32,
    title: String,
    id: String,
}

/// Markdown processor with custom extensions
pub struct MarkdownProcessor {
    options: Options,
}

impl MarkdownProcessor {
    pub fn new() -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        // Note: ENABLE_MATH is NOT enabled - we handle all math delimiters
        // ourselves in MathTransformer to support \[...\], \(...\), $$, and $

        Self { options }
    }

    /// Convert markdown to HTML with all custom transforms
    ///
    /// Returns a tuple of (html, outgoing_links, toc_html)
    pub fn convert(
        &self,
        markdown: &str,
        slug_map: &HashMap<String, String>,
        base_url: &str,
        typst_preamble: Option<&str>,
        citation_context: Option<&CitationContext>,
    ) -> (String, Vec<String>, Option<String>) {
        // Parse markdown into events
        let parser = Parser::new_ext(markdown, self.options);
        let events: Vec<Event> = parser.collect();

        // Collect headings for TOC and later ID injection
        let headings = collect_headings(&events);

        // Transform math delimiters first ($$, $, etc.)
        let math_transformer = MathTransformer::new();
        let events = math_transformer.transform(events);

        // Apply nota blocks (needs paragraph structure intact)
        let nota_transformer = NotaBlockTransformer::new();
        let events = nota_transformer.transform(events);

        // Render math to SVG
        let events = MATH_RENDERER.render_math(events, typst_preamble);

        // Unwrap paragraphs with display math (must be after nota blocks)
        let events = math_transformer.unwrap_display_math_paragraphs(events);

        // Apply sidenote transform
        let sidenote_transformer = SidenoteTransformer::new();
        let events = sidenote_transformer.transform(events);

        // Apply wikilink transform
        let wikilink_transformer = WikilinkTransformer::new(slug_map, base_url);
        let (events, outgoing_links) = wikilink_transformer.transform(events);

        // Apply citation transform
        let mut citation_references = Vec::new();
        let events = if let Some(ctx) = citation_context {
            let transformer = CitationTransformer::new(ctx);
            let (events, refs) = transformer.transform(events);
            citation_references = refs;
            events
        } else {
            events
        };

        // Inject heading ids to match TOC anchors
        let events = attach_heading_ids(events, &headings);
        let events = add_heading_anchors(events);

        // Apply syntax highlighting to code blocks
        let highlight_transformer = HighlightTransformer::new();
        let events = highlight_transformer.transform(events);

        // Convert events to HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        if let Some(refs_html) = render_references(&citation_references) {
            html_output.push('\n');
            html_output.push_str(&refs_html);
        }

        let toc_html = if headings.is_empty() {
            None
        } else {
            Some(render_toc(&headings))
        };

        (html_output, outgoing_links, toc_html)
    }

    /// Convert markdown to HTML without link tracking
    pub fn convert_simple(&self, markdown: &str) -> String {
        let slug_map = HashMap::new();
        let (html, _, _) = self.convert(markdown, &slug_map, "/", None, None);
        html
    }
}

impl Default for MarkdownProcessor {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_headings(events: &[Event]) -> Vec<TocItem> {
    let mut toc = Vec::new();
    let mut current: Option<(u32, String)> = None;

    for event in events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((*level as u32, String::new()));
            }
            Event::Text(text) => {
                if let Some((_level, ref mut title)) = current {
                    title.push_str(text.as_ref());
                }
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some((level, title)) = current.take() {
                    let id = slugify(&title);
                    toc.push(TocItem { level, title, id });
                }
            }
            _ => {}
        }
    }

    toc
}

fn attach_heading_ids(
    mut events: Vec<Event<'static>>,
    headings: &[TocItem],
) -> Vec<Event<'static>> {
    let mut heading_iter = headings.iter();
    let mut result = Vec::with_capacity(events.len());

    for event in events.drain(..) {
        match event {
            Event::Start(Tag::Heading {
                level,
                mut id,
                classes,
                attrs,
            }) => {
                if id.is_none() {
                    if let Some(next) = heading_iter.next() {
                        id = Some(pulldown_cmark::CowStr::Boxed(
                            next.id.clone().into_boxed_str(),
                        ));
                    }
                }
                result.push(Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }));
            }
            _ => result.push(event),
        }
    }

    result
}

fn add_heading_anchors(events: Vec<Event<'static>>) -> Vec<Event<'static>> {
    let mut result = Vec::with_capacity(events.len());
    let mut current_id: Option<String> = None;

    for event in events {
        match event {
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                current_id = id.as_ref().map(|s| s.to_string());
                result.push(Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }));
            }
            Event::End(pulldown_cmark::TagEnd::Heading(level)) => {
                if let Some(id) = current_id.take() {
                    let anchor = format!(
                        "<a class=\"heading-anchor\" href=\"#{}\" aria-label=\"Link to heading\">#</a>",
                        html_escape(&id)
                    );
                    result.push(Event::Html(CowStr::Boxed(anchor.into_boxed_str())));
                }
                result.push(Event::End(pulldown_cmark::TagEnd::Heading(level)));
            }
            other => result.push(other),
        }
    }

    result
}

fn render_toc(headings: &[TocItem]) -> String {
    let mut html = String::from(r#"<nav class="toc-nav"><h3>Contents</h3><ul class="toc-list">"#);
    for h in headings {
        html.push_str(&format!(
            r##"<li class="toc-level-{}"><a href="#{}">{}</a></li>"##,
            h.level,
            h.id,
            html_escape(&h.title)
        ));
    }
    html.push_str("</ul></nav>");
    html
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
    fn test_basic_markdown() {
        let processor = MarkdownProcessor::new();
        let html = processor.convert_simple("# Hello World\n\nThis is a **test**.");
        assert!(html.contains("<h1"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("<strong>test</strong>"));
    }

    #[test]
    fn test_tables() {
        let processor = MarkdownProcessor::new();
        let md = r#"
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
"#;
        let html = processor.convert_simple(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>Header 1</th>"));
    }

    #[test]
    fn test_code_blocks() {
        let processor = MarkdownProcessor::new();
        let md = "```rust\nfn main() {}\n```";
        let html = processor.convert_simple(md);
        assert!(html.contains("<pre"));
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
    }
}
