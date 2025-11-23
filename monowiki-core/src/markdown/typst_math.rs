//! Typst-based math rendering to inline SVG.

use std::{
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::Mutex,
};

use anyhow::{anyhow, Result};
use lru::LruCache;
use once_cell::sync::Lazy;
use pulldown_cmark::{CowStr, Event};
use regex::Regex;
use tracing::warn;
use typst::diag::SourceDiagnostic;
use typst::layout::{Abs, PagedDocument};
use typst_as_lib::TypstEngine;

/// Render math events into inline SVG using Typst.
#[derive(Debug)]
pub struct TypstMathRenderer {
    fonts: Vec<&'static [u8]>,
    cache: Mutex<LruCache<String, String>>,
}

impl TypstMathRenderer {
    pub fn new() -> Self {
        Self {
            fonts: typst_assets::fonts().collect(),
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(MATH_CACHE_CAPACITY).unwrap(),
            )),
        }
    }

    /// Replace `InlineMath` / `DisplayMath` events with raw HTML containing SVG.
    pub fn render_math(
        &self,
        events: Vec<Event<'static>>,
        preamble: Option<&str>,
    ) -> Vec<Event<'static>> {
        events
            .into_iter()
            .map(|event| match event {
                Event::InlineMath(math) => match self.render_math_block(&math, false, preamble) {
                    Ok(html) => Event::InlineHtml(CowStr::Boxed(html.into_boxed_str())),
                    Err(err) => {
                        warn!("Typst inline math failed: {err}");
                        Event::InlineMath(math)
                    }
                },
                Event::DisplayMath(math) => match self.render_math_block(&math, true, preamble) {
                    Ok(html) => Event::Html(CowStr::Boxed(html.into_boxed_str())),
                    Err(err) => {
                        warn!("Typst display math failed: {err}");
                        Event::DisplayMath(math)
                    }
                },
                other => other,
            })
            .collect()
    }

    fn render_math_block(
        &self,
        math: &CowStr<'_>,
        display: bool,
        preamble: Option<&str>,
    ) -> Result<String> {
        let key = cache_key(math, display, preamble);
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(svg) = cache.get(&key) {
                return Ok(wrap_svg(svg, math, display));
            }
        }

        let source = build_source(math, display, preamble);
        let engine = TypstEngine::builder()
            .main_file(source)
            .fonts(self.fonts.iter().copied())
            .build();

        let warned = engine.compile::<PagedDocument>();
        log_warnings(&warned.warnings);
        let doc = warned
            .output
            .map_err(|err| anyhow!("Typst math compilation failed: {err}"))?;

        // Provide minimal padding so strokes aren't clipped at the edges
        // Reduced from 2pt to 0.5pt for tighter inline math rendering
        let svg = typst_svg::svg_merged(&doc, Abs::pt(0.5));
        let svg = normalize_svg(&svg);

        self.cache.lock().unwrap().put(key, svg.clone());

        Ok(wrap_svg(&svg, math, display))
    }
}

impl Default for TypstMathRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn build_source(math: &CowStr<'_>, display: bool, preamble: Option<&str>) -> String {
    let delimiter = if display { "$$" } else { "$" };
    // Use 15pt to match MathJax's typical 1.2-1.3x scaling relative to 15px body text
    // Use medium weight (500) for slightly bolder appearance
    let preamble = preamble.unwrap_or_default();
    format!(
        r#"
#set page(width: auto, height: auto, margin: 0pt, fill: none)
#set text(font: "New Computer Modern", size: 15pt, weight: "medium", fill: black)
#set math.equation(numbering: none)

{preamble}

{delimiter}{math}{delimiter}
"#
    )
}

fn cache_key(math: &CowStr<'_>, display: bool, preamble: Option<&str>) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    math.hash(&mut hasher);
    display.hash(&mut hasher);
    if let Some(preamble) = preamble {
        preamble.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn wrap_svg(svg: &str, math: &CowStr<'_>, display: bool) -> String {
    let alt = html_escape(math);
    if display {
        format!(
            r#"<div class="typst-math typst-display" role="math" aria-label="{alt}" data-math="{alt}" tabindex="0">{svg}<span class="math-source sr-only" aria-hidden="true">{alt}</span></div>"#
        )
    } else {
        format!(
            r#"<span class="typst-math typst-inline" role="math" aria-label="{alt}" data-math="{alt}" tabindex="0">{svg}<span class="math-source sr-only" aria-hidden="true">{alt}</span></span>"#
        )
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn log_warnings(warnings: &[SourceDiagnostic]) {
    for warning in warnings {
        warn!("Typst warning: {warning:?}");
    }
}

fn normalize_svg(svg: &str) -> String {
    let svg = normalize_svg_colors(svg);
    let svg = trim_svg_viewbox(&svg);
    ensure_svg_is_hidden_from_a11y(&svg)
}

fn trim_svg_viewbox(svg: &str) -> String {
    // Extract current viewBox
    static VIEWBOX_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"viewBox="([^"]+)""#).expect("valid viewBox regex")
    });

    let Some(caps) = VIEWBOX_RE.captures(svg) else {
        return svg.to_string();
    };

    let viewbox_str = &caps[1];
    let parts: Vec<&str> = viewbox_str.split_whitespace().collect();
    if parts.len() != 4 {
        return svg.to_string();
    }

    let Ok(orig_width) = parts[2].parse::<f64>() else { return svg.to_string(); };
    let Ok(orig_height) = parts[3].parse::<f64>() else { return svg.to_string(); };

    // Find all text transform matrices to get content bounds
    // Typst uses: <g class="typst-text" transform="matrix(1 0 0 -1 x y)">
    // The y value is the baseline position in the flipped coordinate system
    static TEXT_TRANSFORM_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"class="typst-text"[^>]+transform="matrix\([^)]+\s+([0-9.]+)\)""#)
            .expect("valid text transform regex")
    });

    let mut baseline_y: Option<f64> = None;
    for cap in TEXT_TRANSFORM_RE.captures_iter(svg) {
        if let Some(y_str) = cap.get(1) {
            if let Ok(y) = y_str.as_str().parse::<f64>() {
                baseline_y = Some(baseline_y.map_or(y, |current| current.max(y)));
            }
        }
    }

    let (new_min_y, new_height) = if let Some(baseline) = baseline_y {
        // With 15pt font: cap height ≈ 11pt, descender ≈ 3pt
        // Content spans from (baseline - 11) to (baseline + 3)
        let cap_height = 11.0;
        let descender_depth = 3.0;
        let padding = 1.0;

        let content_top = (baseline - cap_height - padding).max(0.0);
        let content_bottom = baseline + descender_depth + padding;
        let content_height = content_bottom - content_top;

        (content_top, content_height.min(orig_height))
    } else {
        // Fallback: reduce by 40% if we can't find text bounds, keep origin
        (0.0, orig_height * 0.6)
    };

    let new_viewbox = format!("0 {} {} {}", new_min_y, orig_width, new_height);

    // Update viewBox
    let result = VIEWBOX_RE.replace(svg, format!(r#"viewBox="{}""#, new_viewbox));

    // Update height attribute
    static HEIGHT_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"height="[^"]+""#).expect("valid height regex")
    });
    let result = HEIGHT_RE.replace(&result, format!(r#"height="{}pt""#, new_height));

    result.into_owned()
}

fn normalize_svg_colors(svg: &str) -> String {
    // Replace hardcoded black fills/strokes (common from Typst) with currentColor
    static ATTR_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"(?i)(fill|stroke)=["']\s*(black|#000(?:000)?|rgb\(\s*0\s*,\s*0\s*,\s*0\s*\))\s*["']"#)
            .expect("valid color attribute regex")
    });
    static STYLE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r#"(?i)(fill|stroke)\s*:\s*(black|#000(?:000)?|rgb\(\s*0\s*,\s*0\s*,\s*0\s*\))"#,
        )
        .expect("valid color style regex")
    });

    let replaced_attrs = ATTR_RE.replace_all(svg, r#"$1="currentColor""#);
    STYLE_RE
        .replace_all(&replaced_attrs, |caps: &regex::Captures| {
            format!("{}:currentColor", &caps[1])
        })
        .into_owned()
}

fn ensure_svg_is_hidden_from_a11y(svg: &str) -> String {
    if svg.contains("aria-hidden") {
        return svg.to_string();
    }

    // Best-effort injection; if <svg> is not found, fall back to original
    if svg.contains("<svg") {
        return svg
            .replacen("<svg ", "<svg aria-hidden=\"true\" focusable=\"false\" ", 1)
            .replacen("<svg>", "<svg aria-hidden=\"true\" focusable=\"false\">", 1);
    }

    svg.to_string()
}

/// Shared renderer for callers that want a singleton.
pub static MATH_RENDERER: Lazy<TypstMathRenderer> = Lazy::new(TypstMathRenderer::new);

const MATH_CACHE_CAPACITY: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_black_fill_and_stroke_attributes() {
        let input = r##"<svg><path fill="black" stroke="#000"/></svg>"##;
        let out = normalize_svg_colors(input);
        assert!(
            out.contains(r#"fill="currentColor""#),
            "fill should be normalized to currentColor"
        );
        assert!(
            out.contains(r#"stroke="currentColor""#),
            "stroke should be normalized to currentColor"
        );
    }

    #[test]
    fn normalizes_black_fill_and_stroke_styles() {
        let input = r##"<svg><g style="fill:#000;stroke:rgb(0,0,0)"></g></svg>"##;
        let out = normalize_svg_colors(input);
        assert!(
            !out.contains("#000"),
            "style-based hex color should be removed"
        );
        assert!(
            !out.contains("rgb(0,0,0)"),
            "style-based rgb color should be removed"
        );
        assert!(
            out.contains("fill:currentColor"),
            "fill style should be normalized"
        );
        assert!(
            out.contains("stroke:currentColor"),
            "stroke style should be normalized"
        );
    }

    #[test]
    fn injects_aria_hidden_for_svg() {
        let input = r#"<svg width="10" height="10"></svg>"#;
        let out = ensure_svg_is_hidden_from_a11y(input);
        assert!(
            out.contains(r#"aria-hidden="true""#),
            "should inject aria-hidden"
        );
        assert!(
            out.contains(r#"focusable="false""#),
            "should inject focusable=false"
        );
    }

    #[test]
    fn preserves_existing_aria_hidden() {
        let input = r#"<svg aria-hidden="true"></svg>"#;
        let out = ensure_svg_is_hidden_from_a11y(input);
        assert_eq!(out, input);
    }
}
