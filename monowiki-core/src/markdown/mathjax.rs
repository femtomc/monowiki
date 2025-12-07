//! MathJax-compatible math rendering.
//!
//! Converts InlineMath and DisplayMath events to HTML that MathJax can process client-side.

use crate::models::Diagnostic;
use pulldown_cmark::{CowStr, Event};

/// Render math events into MathJax-compatible HTML.
///
/// Unlike the Typst renderer which produces SVG at build time,
/// this simply wraps the LaTeX in delimiters that MathJax will process.
pub fn render_math_for_mathjax(events: Vec<Event<'static>>) -> (Vec<Event<'static>>, Vec<Diagnostic>) {
    let events = events
        .into_iter()
        .map(|event| match event {
            Event::InlineMath(math) => {
                let html = wrap_inline_math(&math);
                Event::InlineHtml(CowStr::Boxed(html.into_boxed_str()))
            }
            Event::DisplayMath(math) => {
                let html = wrap_display_math(&math);
                Event::Html(CowStr::Boxed(html.into_boxed_str()))
            }
            other => other,
        })
        .collect();

    // MathJax handles rendering client-side, so we don't generate diagnostics here
    (events, Vec::new())
}

fn wrap_inline_math(math: &str) -> String {
    let escaped = html_escape(math);
    format!(
        r#"<span class="math math-inline" aria-label="{}">\({}\)</span>"#,
        escaped, math
    )
}

fn wrap_display_math(math: &str) -> String {
    let escaped = html_escape(math);
    format!(
        r#"<div class="math math-display" aria-label="{}">\[{}\]</div>"#,
        escaped, math
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_math_wrapping() {
        let html = wrap_inline_math("x^2 + y^2");
        assert!(html.contains(r"\(x^2 + y^2\)"));
        assert!(html.contains("math-inline"));
    }

    #[test]
    fn test_display_math_wrapping() {
        let html = wrap_display_math(r"\sum_{i=0}^n i");
        assert!(html.contains(r"\[\sum_{i=0}^n i\]"));
        assert!(html.contains("math-display"));
    }

    #[test]
    fn test_escapes_special_chars_in_aria_label() {
        let html = wrap_inline_math("x < y & z > w");
        assert!(html.contains("x &lt; y &amp; z &gt; w"));
    }
}
