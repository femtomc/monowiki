use std::collections::HashMap;
use std::fmt;

/// Attributes for content elements
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attributes {
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub other: HashMap<String, String>,
}

impl Attributes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_class(mut self, class: String) -> Self {
        self.classes.push(class);
        self
    }

    pub fn with_attr(mut self, key: String, value: String) -> Self {
        self.other.insert(key, value);
        self
    }
}

/// Content: the top-level document tree type
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Block(Block),
    Inline(Inline),
    Sequence(Vec<Content>),
}

impl Content {
    /// Combine two content values
    pub fn concat(self, other: Content) -> Content {
        match (self, other) {
            (Content::Sequence(mut seq1), Content::Sequence(seq2)) => {
                seq1.extend(seq2);
                Content::Sequence(seq1)
            }
            (Content::Sequence(mut seq), other) => {
                seq.push(other);
                Content::Sequence(seq)
            }
            (this, Content::Sequence(mut seq)) => {
                seq.insert(0, this);
                Content::Sequence(seq)
            }
            (this, other) => Content::Sequence(vec![this, other]),
        }
    }

    /// Check if this is block-level content
    pub fn is_block(&self) -> bool {
        matches!(self, Content::Block(_))
    }

    /// Check if this is inline content
    pub fn is_inline(&self) -> bool {
        matches!(self, Content::Inline(_))
    }
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Block(b) => write!(f, "{}", b),
            Content::Inline(i) => write!(f, "{}", i),
            Content::Sequence(items) => {
                for item in items {
                    write!(f, "{}", item)?;
                }
                Ok(())
            }
        }
    }
}

/// Block-level content
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        body: Box<Inline>,
        attrs: Attributes,
    },
    Paragraph {
        body: Box<Inline>,
        attrs: Attributes,
    },
    CodeBlock {
        lang: Option<String>,
        code: String,
        opts: HashMap<String, String>,
        attrs: Attributes,
    },
    List {
        items: Vec<ListItem>,
        ordered: bool,
        attrs: Attributes,
    },
    Blockquote {
        body: Box<Content>,
        attrs: Attributes,
    },
    Table {
        headers: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
        attrs: Attributes,
    },
    ThematicBreak {
        attrs: Attributes,
    },
    // Directive is for custom elements
    Directive {
        name: String,
        args: HashMap<String, String>,
        body: Box<Content>,
        attrs: Attributes,
    },
}

impl Block {
    pub fn attrs(&self) -> &Attributes {
        match self {
            Block::Heading { attrs, .. } => attrs,
            Block::Paragraph { attrs, .. } => attrs,
            Block::CodeBlock { attrs, .. } => attrs,
            Block::List { attrs, .. } => attrs,
            Block::Blockquote { attrs, .. } => attrs,
            Block::Table { attrs, .. } => attrs,
            Block::ThematicBreak { attrs } => attrs,
            Block::Directive { attrs, .. } => attrs,
        }
    }

    pub fn attrs_mut(&mut self) -> &mut Attributes {
        match self {
            Block::Heading { attrs, .. } => attrs,
            Block::Paragraph { attrs, .. } => attrs,
            Block::CodeBlock { attrs, .. } => attrs,
            Block::List { attrs, .. } => attrs,
            Block::Blockquote { attrs, .. } => attrs,
            Block::Table { attrs, .. } => attrs,
            Block::ThematicBreak { attrs } => attrs,
            Block::Directive { attrs, .. } => attrs,
        }
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Block::Heading { level, body, .. } => {
                write!(f, "{} {}\n", "#".repeat(*level as usize), body)
            }
            Block::Paragraph { body, .. } => write!(f, "{}\n\n", body),
            Block::CodeBlock { lang, code, .. } => {
                let lang_str = lang.as_deref().unwrap_or("");
                write!(f, "```{}\n{}\n```\n", lang_str, code)
            }
            Block::List { items, ordered, .. } => {
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "- ".to_string()
                    };
                    write!(f, "{}{}\n", marker, item.body)?;
                }
                Ok(())
            }
            Block::Blockquote { body, .. } => write!(f, "> {}\n", body),
            Block::Table { .. } => write!(f, "[table]\n"),
            Block::ThematicBreak { .. } => write!(f, "---\n"),
            Block::Directive { name, body, .. } => write!(f, "!{}[{}]\n", name, body),
        }
    }
}

/// List item
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub body: Inline,
    pub nested: Option<Vec<ListItem>>,
    pub attrs: Attributes,
}

/// Inline content
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Emphasis(Box<Inline>),
    Strong(Box<Inline>),
    Code(String),
    Link {
        body: Box<Inline>,
        url: String,
        title: Option<String>,
    },
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
    Reference(String),
    Math(String),
    Span {
        body: Box<Inline>,
        attrs: Attributes,
    },
    Sequence(Vec<Inline>),
}

impl Inline {
    /// Combine two inline values
    pub fn concat(self, other: Inline) -> Inline {
        match (self, other) {
            (Inline::Sequence(mut seq1), Inline::Sequence(seq2)) => {
                seq1.extend(seq2);
                Inline::Sequence(seq1)
            }
            (Inline::Sequence(mut seq), other) => {
                seq.push(other);
                Inline::Sequence(seq)
            }
            (this, Inline::Sequence(mut seq)) => {
                seq.insert(0, this);
                Inline::Sequence(seq)
            }
            (Inline::Text(s1), Inline::Text(s2)) => Inline::Text(s1 + &s2),
            (this, other) => Inline::Sequence(vec![this, other]),
        }
    }

    /// Create an empty inline
    pub fn empty() -> Inline {
        Inline::Text(String::new())
    }
}

impl fmt::Display for Inline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inline::Text(text) => write!(f, "{}", text),
            Inline::Emphasis(body) => write!(f, "_{}_", body),
            Inline::Strong(body) => write!(f, "*{}*", body),
            Inline::Code(code) => write!(f, "`{}`", code),
            Inline::Link { body, url, .. } => write!(f, "[{}]({})", body, url),
            Inline::Image { alt, url, .. } => write!(f, "![{}]({})", alt, url),
            Inline::Reference(target) => write!(f, "@{}", target),
            Inline::Math(math) => write!(f, "${}$", math),
            Inline::Span { body, .. } => write!(f, "{}", body),
            Inline::Sequence(items) => {
                for item in items {
                    write!(f, "{}", item)?;
                }
                Ok(())
            }
        }
    }
}

/// Helper constructors for content
impl Content {
    pub fn text<S: Into<String>>(s: S) -> Self {
        Content::Inline(Inline::Text(s.into()))
    }

    pub fn paragraph(body: Inline) -> Self {
        Content::Block(Block::Paragraph {
            body: Box::new(body),
            attrs: Attributes::new(),
        })
    }

    pub fn heading(level: u8, body: Inline) -> Self {
        Content::Block(Block::Heading {
            level,
            body: Box::new(body),
            attrs: Attributes::new(),
        })
    }

    pub fn code_block<S: Into<String>>(lang: Option<String>, code: S) -> Self {
        Content::Block(Block::CodeBlock {
            lang,
            code: code.into(),
            opts: HashMap::new(),
            attrs: Attributes::new(),
        })
    }

    pub fn blockquote(body: Content) -> Self {
        Content::Block(Block::Blockquote {
            body: Box::new(body),
            attrs: Attributes::new(),
        })
    }

    pub fn thematic_break() -> Self {
        Content::Block(Block::ThematicBreak {
            attrs: Attributes::new(),
        })
    }
}

impl Inline {
    pub fn text<S: Into<String>>(s: S) -> Self {
        Inline::Text(s.into())
    }

    pub fn emphasis(body: Inline) -> Self {
        Inline::Emphasis(Box::new(body))
    }

    pub fn strong(body: Inline) -> Self {
        Inline::Strong(Box::new(body))
    }

    pub fn code<S: Into<String>>(s: S) -> Self {
        Inline::Code(s.into())
    }

    pub fn link<S: Into<String>>(body: Inline, url: S) -> Self {
        Inline::Link {
            body: Box::new(body),
            url: url.into(),
            title: None,
        }
    }

    pub fn reference<S: Into<String>>(target: S) -> Self {
        Inline::Reference(target.into())
    }

    pub fn math<S: Into<String>>(s: S) -> Self {
        Inline::Math(s.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_concat() {
        let i1 = Inline::text("Hello ");
        let i2 = Inline::text("world");
        let result = i1.concat(i2);
        assert_eq!(result, Inline::text("Hello world"));
    }

    #[test]
    fn test_content_constructors() {
        let para = Content::paragraph(Inline::text("Test"));
        assert!(para.is_block());

        let heading = Content::heading(1, Inline::text("Title"));
        assert!(heading.is_block());

        let text = Content::text("inline");
        assert!(text.is_inline());
    }

    #[test]
    fn test_attributes() {
        let attrs = Attributes::new()
            .with_id("test".to_string())
            .with_class("highlight".to_string())
            .with_attr("key".to_string(), "value".to_string());

        assert_eq!(attrs.id, Some("test".to_string()));
        assert_eq!(attrs.classes, vec!["highlight".to_string()]);
        assert_eq!(attrs.other.get("key"), Some(&"value".to_string()));
    }
}
