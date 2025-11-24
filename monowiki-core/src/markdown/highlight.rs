//! Code syntax highlighting using syntect.

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use std::sync::OnceLock;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| SyntaxSet::load_defaults_newlines())
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let theme_set = ThemeSet::load_defaults();
        // Use a clean, minimal light theme matching monospace aesthetic
        theme_set
            .themes
            .get("InspiredGitHub")
            .or_else(|| theme_set.themes.get("base16-ocean.light"))
            .unwrap()
            .clone()
    })
}

/// Parsed code block info (language and optional title)
struct CodeBlockInfo {
    lang: String,
    title: Option<String>,
}

impl CodeBlockInfo {
    /// Parse info string like `bash title="Installation"` or just `rust`
    fn parse(info: &str) -> Self {
        let info = info.trim();

        // Try to extract title="..." or title='...'
        let title_pattern = regex::Regex::new(r#"title\s*=\s*["']([^"']+)["']"#).ok();
        let title = title_pattern
            .as_ref()
            .and_then(|re| re.captures(info))
            .map(|caps| caps.get(1).unwrap().as_str().to_string());

        // Extract language (first word before any attributes)
        let lang = info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        Self { lang, title }
    }
}

/// Transformer for syntax highlighting code blocks
pub struct HighlightTransformer;

impl HighlightTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Transform events, adding syntax highlighting to code blocks
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut in_code_block = false;
        let mut code_info: Option<CodeBlockInfo> = None;
        let mut code_content = String::new();

        for event in events {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    code_info = Some(CodeBlockInfo::parse(&lang));
                    code_content.clear();
                }
                Event::Text(text) if in_code_block => {
                    code_content.push_str(text.as_ref());
                }
                Event::End(TagEnd::CodeBlock) if in_code_block => {
                    in_code_block = false;

                    // Highlight the code
                    if let Some(info) = &code_info {
                        let highlighted = self.highlight_code(&code_content, &info.lang, info.title.as_deref());
                        result.push(Event::Html(CowStr::Boxed(highlighted.into_boxed_str())));
                    } else {
                        // No language specified, output as plain pre/code
                        result.push(Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)));
                        result.push(Event::Text(CowStr::Boxed(
                            code_content.clone().into_boxed_str(),
                        )));
                        result.push(Event::End(TagEnd::CodeBlock));
                    }

                    code_info = None;
                }
                _ => {
                    result.push(self.event_into_static(event));
                }
            }
        }

        result
    }

    fn highlight_code(&self, code: &str, lang: &str, title: Option<&str>) -> String {
        let ss = syntax_set();
        let syntax = ss
            .find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_extension(lang))
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        let highlighted = match highlighted_html_for_string(code, ss, syntax, theme()) {
            Ok(html) => html,
            Err(_) => {
                // Fallback to plain code block
                format!("<pre><code>{}</code></pre>", html_escape(code))
            }
        };

        // Build toolbar with optional title and copy button
        let title_span = match title {
            Some(t) => format!("<span class=\"code-title\">{}</span>", html_escape(t)),
            None => String::new(),
        };

        format!(
            "<div class=\"code-block\">\n<div class=\"code-toolbar\">{}<button class=\"copy-code-btn\" type=\"button\" aria-label=\"Copy code\">Copy</button></div>\n{}\n</div>",
            title_span,
            highlighted
        )
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
                CodeBlockKind::Indented => CodeBlockKind::Indented,
                CodeBlockKind::Fenced(lang) => {
                    CodeBlockKind::Fenced(CowStr::Boxed(lang.to_string().into_boxed_str()))
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

impl Default for HighlightTransformer {
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
