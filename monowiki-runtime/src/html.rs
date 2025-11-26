//! HTML renderer for MRL Content
//!
//! Converts monowiki-mrl Content trees to HTML strings for live cell output.

use monowiki_mrl::{Attributes, Block, Content, Inline, ListItem};

/// Render Content to HTML string
pub fn render_content(content: &Content) -> String {
    let mut output = String::new();
    render_content_to(&mut output, content);
    output
}

fn render_content_to(out: &mut String, content: &Content) {
    match content {
        Content::Block(block) => render_block_to(out, block),
        Content::Inline(inline) => render_inline_to(out, inline),
        Content::Sequence(items) => {
            for item in items {
                render_content_to(out, item);
            }
        }
    }
}

fn render_block_to(out: &mut String, block: &Block) {
    match block {
        Block::Heading { level, body, attrs } => {
            let tag = format!("h{}", level.min(&6));
            out.push('<');
            out.push_str(&tag);
            render_attrs_to(out, attrs);
            out.push('>');
            render_inline_to(out, body);
            out.push_str("</");
            out.push_str(&tag);
            out.push_str(">\n");
        }
        Block::Paragraph { body, attrs } => {
            out.push_str("<p");
            render_attrs_to(out, attrs);
            out.push('>');
            render_inline_to(out, body);
            out.push_str("</p>\n");
        }
        Block::CodeBlock {
            lang,
            code,
            attrs,
            ..
        } => {
            out.push_str("<pre");
            render_attrs_to(out, attrs);
            out.push_str("><code");
            if let Some(lang) = lang {
                out.push_str(" class=\"language-");
                out.push_str(&escape_attr(lang));
                out.push('"');
            }
            out.push('>');
            out.push_str(&escape_html(code));
            out.push_str("</code></pre>\n");
        }
        Block::List {
            items,
            ordered,
            attrs,
        } => {
            let tag = if *ordered { "ol" } else { "ul" };
            out.push('<');
            out.push_str(tag);
            render_attrs_to(out, attrs);
            out.push_str(">\n");
            for item in items {
                render_list_item_to(out, item);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push_str(">\n");
        }
        Block::Blockquote { body, attrs } => {
            out.push_str("<blockquote");
            render_attrs_to(out, attrs);
            out.push_str(">\n");
            render_content_to(out, body);
            out.push_str("</blockquote>\n");
        }
        Block::Table {
            headers,
            rows,
            attrs,
        } => {
            out.push_str("<table");
            render_attrs_to(out, attrs);
            out.push_str(">\n<thead><tr>\n");
            for header in headers {
                out.push_str("<th>");
                render_inline_to(out, header);
                out.push_str("</th>\n");
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            for row in rows {
                out.push_str("<tr>\n");
                for cell in row {
                    out.push_str("<td>");
                    render_inline_to(out, cell);
                    out.push_str("</td>\n");
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody></table>\n");
        }
        Block::ThematicBreak { attrs } => {
            out.push_str("<hr");
            render_attrs_to(out, attrs);
            out.push_str(">\n");
        }
        Block::Directive {
            name, args, body, attrs,
        } => {
            // Render directives as a div with data attributes
            out.push_str("<div");
            render_attrs_to(out, attrs);
            out.push_str(" data-directive=\"");
            out.push_str(&escape_attr(name));
            out.push('"');
            for (key, value) in args {
                out.push_str(" data-");
                out.push_str(&escape_attr(key));
                out.push_str("=\"");
                out.push_str(&escape_attr(value));
                out.push('"');
            }
            out.push_str(">\n");
            render_content_to(out, body);
            out.push_str("</div>\n");
        }
    }
}

fn render_list_item_to(out: &mut String, item: &ListItem) {
    out.push_str("<li");
    render_attrs_to(out, &item.attrs);
    out.push('>');
    render_inline_to(out, &item.body);
    if let Some(nested) = &item.nested {
        out.push_str("\n<ul>\n");
        for nested_item in nested {
            render_list_item_to(out, nested_item);
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</li>\n");
}

fn render_inline_to(out: &mut String, inline: &Inline) {
    match inline {
        Inline::Text(text) => {
            out.push_str(&escape_html(text));
        }
        Inline::Emphasis(body) => {
            out.push_str("<em>");
            render_inline_to(out, body);
            out.push_str("</em>");
        }
        Inline::Strong(body) => {
            out.push_str("<strong>");
            render_inline_to(out, body);
            out.push_str("</strong>");
        }
        Inline::Code(code) => {
            out.push_str("<code>");
            out.push_str(&escape_html(code));
            out.push_str("</code>");
        }
        Inline::Link { body, url, title } => {
            out.push_str("<a href=\"");
            out.push_str(&escape_attr(url));
            out.push('"');
            if let Some(title) = title {
                out.push_str(" title=\"");
                out.push_str(&escape_attr(title));
                out.push('"');
            }
            out.push('>');
            render_inline_to(out, body);
            out.push_str("</a>");
        }
        Inline::Image { alt, url, title } => {
            out.push_str("<img src=\"");
            out.push_str(&escape_attr(url));
            out.push_str("\" alt=\"");
            out.push_str(&escape_attr(alt));
            out.push('"');
            if let Some(title) = title {
                out.push_str(" title=\"");
                out.push_str(&escape_attr(title));
                out.push('"');
            }
            out.push('>');
        }
        Inline::Reference(target) => {
            // Render references as links (to be resolved by the frontend)
            out.push_str("<a class=\"reference\" data-ref=\"");
            out.push_str(&escape_attr(target));
            out.push_str("\">");
            out.push_str(&escape_html(target));
            out.push_str("</a>");
        }
        Inline::Math(math) => {
            // Render math with a class for client-side rendering (e.g., KaTeX)
            out.push_str("<span class=\"math\">");
            out.push_str(&escape_html(math));
            out.push_str("</span>");
        }
        Inline::Span { body, attrs } => {
            out.push_str("<span");
            render_attrs_to(out, attrs);
            out.push('>');
            render_inline_to(out, body);
            out.push_str("</span>");
        }
        Inline::Sequence(items) => {
            for item in items {
                render_inline_to(out, item);
            }
        }
    }
}

fn render_attrs_to(out: &mut String, attrs: &Attributes) {
    if let Some(id) = &attrs.id {
        out.push_str(" id=\"");
        out.push_str(&escape_attr(id));
        out.push('"');
    }
    if !attrs.classes.is_empty() {
        out.push_str(" class=\"");
        for (i, class) in attrs.classes.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&escape_attr(class));
        }
        out.push('"');
    }
    for (key, value) in &attrs.other {
        out.push(' ');
        out.push_str(&escape_attr(key));
        out.push_str("=\"");
        out.push_str(&escape_attr(value));
        out.push('"');
    }
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_paragraph() {
        let content = Content::paragraph(Inline::text("Hello, world!"));
        let html = render_content(&content);
        assert_eq!(html, "<p>Hello, world!</p>\n");
    }

    #[test]
    fn test_render_heading() {
        let content = Content::heading(1, Inline::text("Title"));
        let html = render_content(&content);
        assert_eq!(html, "<h1>Title</h1>\n");
    }

    #[test]
    fn test_render_emphasis() {
        let content = Content::Inline(Inline::emphasis(Inline::text("important")));
        let html = render_content(&content);
        assert_eq!(html, "<em>important</em>");
    }

    #[test]
    fn test_render_strong() {
        let content = Content::Inline(Inline::strong(Inline::text("bold")));
        let html = render_content(&content);
        assert_eq!(html, "<strong>bold</strong>");
    }

    #[test]
    fn test_render_code_block() {
        let content = Content::code_block(Some("rust".to_string()), "fn main() {}");
        let html = render_content(&content);
        assert!(html.contains("<pre><code class=\"language-rust\">"));
        assert!(html.contains("fn main() {}"));
    }

    #[test]
    fn test_escape_html() {
        let content = Content::Inline(Inline::text("<script>alert('xss')</script>"));
        let html = render_content(&content);
        assert_eq!(html, "&lt;script&gt;alert('xss')&lt;/script&gt;");
    }

    #[test]
    fn test_render_link() {
        let content = Content::Inline(Inline::link(
            Inline::text("click here"),
            "https://example.com",
        ));
        let html = render_content(&content);
        assert_eq!(
            html,
            "<a href=\"https://example.com\">click here</a>"
        );
    }

    #[test]
    fn test_render_sequence() {
        let content = Content::Sequence(vec![
            Content::heading(1, Inline::text("Title")),
            Content::paragraph(Inline::text("Body text")),
        ]);
        let html = render_content(&content);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<p>Body text</p>"));
    }
}
