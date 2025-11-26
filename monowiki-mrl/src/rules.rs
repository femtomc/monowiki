//! Show/Set rule system for content transformation
//!
//! This module implements the rule-based transformation system that allows
//! users to customize how content is rendered without modifying the source.
//!
//! Show rules transform content of a given type into new content.
//! Set rules modify attributes/properties of content elements.

use crate::content::{Attributes, Block, Content, Inline};
use crate::error::{MrlError, Result, Span};
use crate::shrubbery::{Shrubbery, Symbol};
use std::collections::HashMap;

/// A selector that matches content elements
#[derive(Debug, Clone)]
pub struct Selector {
    /// The base type to match (e.g., "heading", "paragraph", "emphasis")
    pub base: SelectorBase,
    /// Optional predicate for filtering (e.g., level == 1)
    pub predicate: Option<Predicate>,
}

/// Base selector types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorBase {
    // Block types
    Heading,
    Paragraph,
    CodeBlock,
    List,
    Blockquote,
    Table,
    ThematicBreak,
    Directive,
    // Inline types
    Text,
    Emphasis,
    Strong,
    Code,
    Link,
    Image,
    Reference,
    Math,
    Span,
    // Generic
    Block,
    Inline,
    Content,
}

impl SelectorBase {
    /// Parse a selector base from a symbol name
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "heading" => Some(SelectorBase::Heading),
            "paragraph" | "para" | "p" => Some(SelectorBase::Paragraph),
            "codeblock" | "code-block" => Some(SelectorBase::CodeBlock),
            "list" => Some(SelectorBase::List),
            "blockquote" | "quote" => Some(SelectorBase::Blockquote),
            "table" => Some(SelectorBase::Table),
            "thematicbreak" | "hr" => Some(SelectorBase::ThematicBreak),
            "directive" => Some(SelectorBase::Directive),
            "text" => Some(SelectorBase::Text),
            "emphasis" | "emph" | "em" => Some(SelectorBase::Emphasis),
            "strong" | "bold" | "b" => Some(SelectorBase::Strong),
            "code" => Some(SelectorBase::Code),
            "link" | "a" => Some(SelectorBase::Link),
            "image" | "img" => Some(SelectorBase::Image),
            "reference" | "ref" => Some(SelectorBase::Reference),
            "math" => Some(SelectorBase::Math),
            "span" => Some(SelectorBase::Span),
            "block" => Some(SelectorBase::Block),
            "inline" => Some(SelectorBase::Inline),
            "content" => Some(SelectorBase::Content),
            _ => None,
        }
    }

    /// Check if this selector matches the given content
    pub fn matches(&self, content: &Content) -> bool {
        match (self, content) {
            // Exact block matches
            (SelectorBase::Heading, Content::Block(Block::Heading { .. })) => true,
            (SelectorBase::Paragraph, Content::Block(Block::Paragraph { .. })) => true,
            (SelectorBase::CodeBlock, Content::Block(Block::CodeBlock { .. })) => true,
            (SelectorBase::List, Content::Block(Block::List { .. })) => true,
            (SelectorBase::Blockquote, Content::Block(Block::Blockquote { .. })) => true,
            (SelectorBase::Table, Content::Block(Block::Table { .. })) => true,
            (SelectorBase::ThematicBreak, Content::Block(Block::ThematicBreak { .. })) => true,
            (SelectorBase::Directive, Content::Block(Block::Directive { .. })) => true,
            // Exact inline matches
            (SelectorBase::Text, Content::Inline(Inline::Text(_))) => true,
            (SelectorBase::Emphasis, Content::Inline(Inline::Emphasis(_))) => true,
            (SelectorBase::Strong, Content::Inline(Inline::Strong(_))) => true,
            (SelectorBase::Code, Content::Inline(Inline::Code(_))) => true,
            (SelectorBase::Link, Content::Inline(Inline::Link { .. })) => true,
            (SelectorBase::Image, Content::Inline(Inline::Image { .. })) => true,
            (SelectorBase::Reference, Content::Inline(Inline::Reference(_))) => true,
            (SelectorBase::Math, Content::Inline(Inline::Math(_))) => true,
            (SelectorBase::Span, Content::Inline(Inline::Span { .. })) => true,
            // Generic matches
            (SelectorBase::Block, Content::Block(_)) => true,
            (SelectorBase::Inline, Content::Inline(_)) => true,
            (SelectorBase::Content, _) => true,
            _ => false,
        }
    }
}

/// Predicate for filtering matched elements
#[derive(Debug, Clone)]
pub enum Predicate {
    /// Compare a field to a value: field == value
    Equals(String, PredicateValue),
    /// Compare a field to a value: field != value
    NotEquals(String, PredicateValue),
    /// Compare a field to a value: field < value
    LessThan(String, PredicateValue),
    /// Compare a field to a value: field <= value
    LessOrEqual(String, PredicateValue),
    /// Compare a field to a value: field > value
    GreaterThan(String, PredicateValue),
    /// Compare a field to a value: field >= value
    GreaterOrEqual(String, PredicateValue),
    /// Logical AND of predicates
    And(Box<Predicate>, Box<Predicate>),
    /// Logical OR of predicates
    Or(Box<Predicate>, Box<Predicate>),
    /// Logical NOT of predicate
    Not(Box<Predicate>),
}

/// Values that can be compared in predicates
#[derive(Debug, Clone)]
pub enum PredicateValue {
    Int(i64),
    String(String),
    Bool(bool),
}

impl Predicate {
    /// Evaluate this predicate against a content element
    pub fn evaluate(&self, content: &Content) -> bool {
        match self {
            Predicate::Equals(field, value) => {
                if let Some(field_val) = get_field(content, field) {
                    field_val == *value
                } else {
                    false
                }
            }
            Predicate::NotEquals(field, value) => {
                if let Some(field_val) = get_field(content, field) {
                    field_val != *value
                } else {
                    true
                }
            }
            Predicate::LessThan(field, value) => {
                compare_field(content, field, value, |a, b| a < b)
            }
            Predicate::LessOrEqual(field, value) => {
                compare_field(content, field, value, |a, b| a <= b)
            }
            Predicate::GreaterThan(field, value) => {
                compare_field(content, field, value, |a, b| a > b)
            }
            Predicate::GreaterOrEqual(field, value) => {
                compare_field(content, field, value, |a, b| a >= b)
            }
            Predicate::And(left, right) => left.evaluate(content) && right.evaluate(content),
            Predicate::Or(left, right) => left.evaluate(content) || right.evaluate(content),
            Predicate::Not(inner) => !inner.evaluate(content),
        }
    }
}

impl PartialEq for PredicateValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PredicateValue::Int(a), PredicateValue::Int(b)) => a == b,
            (PredicateValue::String(a), PredicateValue::String(b)) => a == b,
            (PredicateValue::Bool(a), PredicateValue::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for PredicateValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (PredicateValue::Int(a), PredicateValue::Int(b)) => a.partial_cmp(b),
            (PredicateValue::String(a), PredicateValue::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

/// Get a field value from content for predicate evaluation
fn get_field(content: &Content, field: &str) -> Option<PredicateValue> {
    match content {
        Content::Block(block) => get_block_field(block, field),
        Content::Inline(inline) => get_inline_field(inline, field),
        Content::Sequence(_) => None,
    }
}

fn get_block_field(block: &Block, field: &str) -> Option<PredicateValue> {
    match (block, field) {
        (Block::Heading { level, .. }, "level") => Some(PredicateValue::Int(*level as i64)),
        (Block::CodeBlock { lang, .. }, "lang" | "language") => {
            lang.as_ref().map(|l| PredicateValue::String(l.clone()))
        }
        (Block::List { ordered, .. }, "ordered") => Some(PredicateValue::Bool(*ordered)),
        (Block::Directive { name, .. }, "name") => Some(PredicateValue::String(name.clone())),
        _ => None,
    }
}

fn get_inline_field(inline: &Inline, field: &str) -> Option<PredicateValue> {
    match (inline, field) {
        (Inline::Link { url, .. }, "url" | "href") => Some(PredicateValue::String(url.clone())),
        (Inline::Link { title, .. }, "title") => {
            title.as_ref().map(|t| PredicateValue::String(t.clone()))
        }
        (Inline::Image { url, .. }, "url" | "src") => Some(PredicateValue::String(url.clone())),
        (Inline::Image { alt, .. }, "alt") => Some(PredicateValue::String(alt.clone())),
        (Inline::Reference(r), "target") => Some(PredicateValue::String(r.clone())),
        _ => None,
    }
}

fn compare_field<F>(content: &Content, field: &str, value: &PredicateValue, cmp: F) -> bool
where
    F: Fn(&PredicateValue, &PredicateValue) -> bool,
{
    if let Some(field_val) = get_field(content, field) {
        cmp(&field_val, value)
    } else {
        false
    }
}

impl Selector {
    /// Create a new selector with just a base type
    pub fn new(base: SelectorBase) -> Self {
        Selector {
            base,
            predicate: None,
        }
    }

    /// Add a predicate to this selector
    pub fn with_predicate(mut self, predicate: Predicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// Check if this selector matches the given content
    pub fn matches(&self, content: &Content) -> bool {
        if !self.base.matches(content) {
            return false;
        }
        if let Some(pred) = &self.predicate {
            pred.evaluate(content)
        } else {
            true
        }
    }

    /// Parse a selector from shrubbery
    pub fn from_shrubbery(shrub: &Shrubbery, symbols: &HashMap<u64, String>) -> Option<Self> {
        match shrub {
            Shrubbery::Selector { base, predicate, .. } => {
                let base_name = symbols.get(&base.id())?;
                let base = SelectorBase::from_name(base_name)?;
                // TODO: Parse predicate from shrubbery
                Some(Selector::new(base))
            }
            Shrubbery::Identifier(sym, _, _) => {
                let name = symbols.get(&sym.id())?;
                let base = SelectorBase::from_name(name)?;
                Some(Selector::new(base))
            }
            _ => None,
        }
    }
}

/// A show rule: transforms matching content into new content
#[derive(Debug, Clone)]
pub struct ShowRule {
    /// Selector for matching elements
    pub selector: Selector,
    /// Transform function (takes matched element via `it` binding)
    pub transform: Box<Shrubbery>,
    /// Source span for error reporting
    pub span: Span,
}

/// A set rule: modifies attributes of matching content
#[derive(Debug, Clone)]
pub struct SetRule {
    /// Selector for matching elements
    pub selector: Selector,
    /// Properties to set (attribute name -> value)
    pub properties: HashMap<String, SetValue>,
    /// Source span for error reporting
    pub span: Span,
}

/// Value that can be set by a set rule
#[derive(Debug, Clone)]
pub enum SetValue {
    String(String),
    Int(i64),
    Bool(bool),
}

/// Collection of active rules
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    /// Show rules in order of definition (later rules take precedence)
    pub show_rules: Vec<ShowRule>,
    /// Set rules in order of definition
    pub set_rules: Vec<SetRule>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a show rule
    pub fn add_show_rule(&mut self, rule: ShowRule) {
        self.show_rules.push(rule);
    }

    /// Add a set rule
    pub fn add_set_rule(&mut self, rule: SetRule) {
        self.set_rules.push(rule);
    }

    /// Check if there are any rules
    pub fn is_empty(&self) -> bool {
        self.show_rules.is_empty() && self.set_rules.is_empty()
    }

    /// Apply set rules to content, modifying attributes
    pub fn apply_set_rules(&self, content: &mut Content) {
        for rule in &self.set_rules {
            apply_set_rule_recursive(content, rule);
        }
    }
}

/// Apply a set rule recursively to content
fn apply_set_rule_recursive(content: &mut Content, rule: &SetRule) {
    // First check if this content matches
    if rule.selector.matches(content) {
        apply_set_properties(content, &rule.properties);
    }

    // Then recurse into children
    match content {
        Content::Block(block) => {
            apply_set_to_block_children(block, rule);
        }
        Content::Inline(inline) => {
            apply_set_to_inline_children(inline, rule);
        }
        Content::Sequence(items) => {
            for item in items {
                apply_set_rule_recursive(item, rule);
            }
        }
    }
}

fn apply_set_properties(content: &mut Content, properties: &HashMap<String, SetValue>) {
    match content {
        Content::Block(block) => {
            let attrs = block.attrs_mut();
            for (key, value) in properties {
                match value {
                    SetValue::String(s) => {
                        if key == "id" {
                            attrs.id = Some(s.clone());
                        } else if key == "class" {
                            attrs.classes.push(s.clone());
                        } else {
                            attrs.other.insert(key.clone(), s.clone());
                        }
                    }
                    SetValue::Int(i) => {
                        attrs.other.insert(key.clone(), i.to_string());
                    }
                    SetValue::Bool(b) => {
                        attrs.other.insert(key.clone(), b.to_string());
                    }
                }
            }
        }
        Content::Inline(Inline::Span { attrs, .. }) => {
            for (key, value) in properties {
                match value {
                    SetValue::String(s) => {
                        if key == "id" {
                            attrs.id = Some(s.clone());
                        } else if key == "class" {
                            attrs.classes.push(s.clone());
                        } else {
                            attrs.other.insert(key.clone(), s.clone());
                        }
                    }
                    SetValue::Int(i) => {
                        attrs.other.insert(key.clone(), i.to_string());
                    }
                    SetValue::Bool(b) => {
                        attrs.other.insert(key.clone(), b.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

fn apply_set_to_block_children(block: &mut Block, rule: &SetRule) {
    match block {
        Block::Blockquote { body, .. } => {
            apply_set_rule_recursive(body, rule);
        }
        Block::Directive { body, .. } => {
            apply_set_rule_recursive(body, rule);
        }
        _ => {}
    }
}

fn apply_set_to_inline_children(inline: &mut Inline, rule: &SetRule) {
    match inline {
        Inline::Emphasis(inner) | Inline::Strong(inner) => {
            let mut content = Content::Inline(*inner.clone());
            apply_set_rule_recursive(&mut content, rule);
            if let Content::Inline(new_inner) = content {
                **inner = new_inner;
            }
        }
        Inline::Link { body, .. } | Inline::Span { body, .. } => {
            let mut content = Content::Inline(*body.clone());
            apply_set_rule_recursive(&mut content, rule);
            if let Content::Inline(new_body) = content {
                **body = new_body;
            }
        }
        Inline::Sequence(items) => {
            for item in items {
                let mut content = Content::Inline(item.clone());
                apply_set_rule_recursive(&mut content, rule);
                if let Content::Inline(new_item) = content {
                    *item = new_item;
                }
            }
        }
        _ => {}
    }
}

/// Apply show rules by transforming content
/// Returns transformed content and whether any transformation occurred
pub fn apply_show_rules<F>(
    content: Content,
    rules: &[ShowRule],
    transform_fn: &mut F,
) -> Result<Content>
where
    F: FnMut(&Content, &Shrubbery) -> Result<Content>,
{
    // Find first matching rule (last defined takes precedence, so iterate in reverse)
    for rule in rules.iter().rev() {
        if rule.selector.matches(&content) {
            // Apply the transform with `it` bound to the matched content
            return transform_fn(&content, &rule.transform);
        }
    }

    // No matching rule - recurse into children
    match content {
        Content::Block(block) => {
            let transformed = apply_show_rules_to_block(block, rules, transform_fn)?;
            Ok(Content::Block(transformed))
        }
        Content::Inline(inline) => {
            let transformed = apply_show_rules_to_inline(inline, rules, transform_fn)?;
            Ok(Content::Inline(transformed))
        }
        Content::Sequence(items) => {
            let transformed: Result<Vec<_>> = items
                .into_iter()
                .map(|item| apply_show_rules(item, rules, transform_fn))
                .collect();
            Ok(Content::Sequence(transformed?))
        }
    }
}

fn apply_show_rules_to_block<F>(
    block: Block,
    rules: &[ShowRule],
    transform_fn: &mut F,
) -> Result<Block>
where
    F: FnMut(&Content, &Shrubbery) -> Result<Content>,
{
    match block {
        Block::Heading { level, body, attrs } => {
            let transformed = apply_show_rules_to_inline(*body, rules, transform_fn)?;
            Ok(Block::Heading {
                level,
                body: Box::new(transformed),
                attrs,
            })
        }
        Block::Paragraph { body, attrs } => {
            let transformed = apply_show_rules_to_inline(*body, rules, transform_fn)?;
            Ok(Block::Paragraph {
                body: Box::new(transformed),
                attrs,
            })
        }
        Block::Blockquote { body, attrs } => {
            let transformed = apply_show_rules(*body, rules, transform_fn)?;
            Ok(Block::Blockquote {
                body: Box::new(transformed),
                attrs,
            })
        }
        Block::Directive {
            name,
            args,
            body,
            attrs,
        } => {
            let transformed = apply_show_rules(*body, rules, transform_fn)?;
            Ok(Block::Directive {
                name,
                args,
                body: Box::new(transformed),
                attrs,
            })
        }
        Block::List { items, ordered, attrs } => {
            let transformed_items: Result<Vec<_>> = items
                .into_iter()
                .map(|item| {
                    let body = apply_show_rules_to_inline(item.body, rules, transform_fn)?;
                    Ok(crate::content::ListItem {
                        body,
                        nested: item.nested, // TODO: recurse into nested
                        attrs: item.attrs,
                    })
                })
                .collect();
            Ok(Block::List {
                items: transformed_items?,
                ordered,
                attrs,
            })
        }
        // Pass through other blocks unchanged
        other => Ok(other),
    }
}

fn apply_show_rules_to_inline<F>(
    inline: Inline,
    rules: &[ShowRule],
    transform_fn: &mut F,
) -> Result<Inline>
where
    F: FnMut(&Content, &Shrubbery) -> Result<Content>,
{
    // First check if the inline itself matches any show rule
    let content = Content::Inline(inline.clone());
    for rule in rules.iter().rev() {
        if rule.selector.matches(&content) {
            let transformed = transform_fn(&content, &rule.transform)?;
            return match transformed {
                Content::Inline(i) => Ok(i),
                _ => Err(MrlError::ExpansionError {
                    span: rule.span,
                    message: "Show rule for inline must produce inline content".to_string(),
                }),
            };
        }
    }

    // No match - recurse into children
    match inline {
        Inline::Emphasis(inner) => {
            let transformed = apply_show_rules_to_inline(*inner, rules, transform_fn)?;
            Ok(Inline::Emphasis(Box::new(transformed)))
        }
        Inline::Strong(inner) => {
            let transformed = apply_show_rules_to_inline(*inner, rules, transform_fn)?;
            Ok(Inline::Strong(Box::new(transformed)))
        }
        Inline::Link { body, url, title } => {
            let transformed = apply_show_rules_to_inline(*body, rules, transform_fn)?;
            Ok(Inline::Link {
                body: Box::new(transformed),
                url,
                title,
            })
        }
        Inline::Span { body, attrs } => {
            let transformed = apply_show_rules_to_inline(*body, rules, transform_fn)?;
            Ok(Inline::Span {
                body: Box::new(transformed),
                attrs,
            })
        }
        Inline::Sequence(items) => {
            let transformed: Result<Vec<_>> = items
                .into_iter()
                .map(|item| apply_show_rules_to_inline(item, rules, transform_fn))
                .collect();
            Ok(Inline::Sequence(transformed?))
        }
        // Pass through other inlines unchanged
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_base_matching() {
        let heading = Content::Block(Block::Heading {
            level: 1,
            body: Box::new(Inline::Text("Test".to_string())),
            attrs: Attributes::default(),
        });

        assert!(SelectorBase::Heading.matches(&heading));
        assert!(SelectorBase::Block.matches(&heading));
        assert!(SelectorBase::Content.matches(&heading));
        assert!(!SelectorBase::Paragraph.matches(&heading));
        assert!(!SelectorBase::Inline.matches(&heading));
    }

    #[test]
    fn test_selector_with_predicate() {
        let h1 = Content::Block(Block::Heading {
            level: 1,
            body: Box::new(Inline::Text("Title".to_string())),
            attrs: Attributes::default(),
        });

        let h2 = Content::Block(Block::Heading {
            level: 2,
            body: Box::new(Inline::Text("Subtitle".to_string())),
            attrs: Attributes::default(),
        });

        let selector = Selector::new(SelectorBase::Heading)
            .with_predicate(Predicate::Equals("level".to_string(), PredicateValue::Int(1)));

        assert!(selector.matches(&h1));
        assert!(!selector.matches(&h2));
    }

    #[test]
    fn test_set_rule_application() {
        let mut content = Content::Block(Block::Paragraph {
            body: Box::new(Inline::Text("Hello".to_string())),
            attrs: Attributes::default(),
        });

        let mut properties = HashMap::new();
        properties.insert("class".to_string(), SetValue::String("intro".to_string()));

        let rule = SetRule {
            selector: Selector::new(SelectorBase::Paragraph),
            properties,
            span: Span::default(),
        };

        let mut rules = RuleSet::new();
        rules.add_set_rule(rule);
        rules.apply_set_rules(&mut content);

        if let Content::Block(Block::Paragraph { attrs, .. }) = &content {
            assert!(attrs.classes.contains(&"intro".to_string()));
        } else {
            panic!("Expected paragraph");
        }
    }
}
