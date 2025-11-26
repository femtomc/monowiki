//! Pattern matching infrastructure for the Sammy actor runtime.
//!
//! This module provides:
//! - Generic tree pattern matching with ellipsis support
//! - Runtime pattern serialization via preserves
//! - Trait-based datum matching abstraction

mod tree;

pub use tree::*;

use preserves::IOValue;
use preserves::types::{CompoundClass, ValueClass};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

/// Trait implemented by literal types that can surface human-friendly hints.
pub trait LiteralLabel {
    fn literal_string(&self) -> Option<String> {
        None
    }

    fn literal_symbol(&self) -> Option<String> {
        None
    }

    fn literal_hint(&self) -> Option<String> {
        self.literal_string().or_else(|| self.literal_symbol())
    }
}

impl LiteralLabel for IOValue {
    fn literal_string(&self) -> Option<String> {
        self.as_string().map(|s| s.to_string())
    }

    fn literal_symbol(&self) -> Option<String> {
        self.as_symbol().map(|sym| sym.as_ref().to_string())
    }
}

/// Source attribution for guards and predicate obligations produced during pattern normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardSource {
    Explicit,
    Trait { name: String },
    PredicateSugar { name: String },
}

/// Structured description of a predicate that must hold after structural matching succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateObligation {
    pub predicate: String,
    pub binding: String,
    pub source: GuardSource,
}

impl PredicateObligation {
    pub fn new(predicate: String, binding: String, source: GuardSource) -> Self {
        Self {
            predicate,
            binding,
            source,
        }
    }
}

impl MatchDatum<IOValue> for IOValue {
    fn literal_matches(&self, literal: &IOValue) -> bool {
        self == literal
    }

    fn list_children(&self) -> Option<(Vec<Self>, Option<Self>)> {
        sequence_children(self).map(|children| (children, None))
    }

    fn vector_children(&self) -> Option<Vec<Self>> {
        sequence_children(self)
    }

    fn record_fields(&self) -> Option<(String, Vec<Self>)> {
        if !self.is_record() {
            return None;
        }
        let label = self
            .label()
            .as_symbol()
            .map(|sym| sym.as_ref().to_string())
            .unwrap_or_else(|| "<record>".to_string());
        let fields = self.iter().map(|field| IOValue::from(field)).collect();
        Some((label, fields))
    }

    fn set_items(&self) -> Option<Vec<Self>> {
        let inner = self.value();
        match inner.value_class() {
            ValueClass::Compound(CompoundClass::Set) => Some(
                inner
                    .iter()
                    .map(|value| IOValue::from(value))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    fn dict_entries(&self) -> Option<Vec<(String, Self)>> {
        let inner = self.value();
        match inner.value_class() {
            ValueClass::Compound(CompoundClass::Dictionary) => {
                if let Some(dict) = inner.as_dictionary() {
                    let mut entries = Vec::new();
                    for (key, value) in dict {
                        let key_text = if let Some(sym) = key.as_symbol() {
                            sym.as_ref().to_string()
                        } else if let Some(text) = key.as_string() {
                            text.to_string()
                        } else {
                            "<key>".to_string()
                        };
                        entries.push((key_text, IOValue::from(value)));
                    }
                    Some(entries)
                } else {
                    Some(Vec::new())
                }
            }
            _ => None,
        }
    }

    fn string_value(&self) -> Option<String> {
        if let Some(text) = self.as_string() {
            return Some(text.to_string());
        }
        if let Some(symbol) = self.as_symbol() {
            let raw = symbol.as_ref().to_string();
            return Some(raw.trim_start_matches(':').to_string());
        }
        None
    }
}

fn sequence_children(value: &IOValue) -> Option<Vec<IOValue>> {
    let inner = value.value();
    match inner.value_class() {
        ValueClass::Compound(CompoundClass::Sequence) => Some(
            inner
                .iter()
                .map(|child| IOValue::from(child))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    }
}

const TAG_WILDCARD: &str = ":pattern/wildcard";
const TAG_LITERAL: &str = ":pattern/literal";
const TAG_RECORD: &str = ":pattern/record";
const TAG_BIND: &str = ":pattern/bind";
const TAG_SEQUENCE: &str = ":pattern/sequence";
const TAG_LIST: &str = ":pattern/list";
const TAG_SET: &str = ":pattern/set";
const TAG_DICT: &str = ":pattern/dict";
const TAG_DICT_ENTRY_LABEL: &str = ":pattern/dict-entry";
const TAG_GUARD: &str = ":pattern/guard";
const TAG_STRING_PREFIX: &str = ":pattern/string-prefix";
const TAG_REPEAT: &str = ":pattern/repeat";

/// Canonical pattern representation shared between MRL and the runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "L: Serialize", deserialize = "L: Deserialize<'de>"))]
pub enum Pattern<L = IOValue> {
    Wildcard,
    Literal(L),
    Record {
        label: String,
        fields: Vec<Pattern<L>>,
    },
    Bind {
        name: String,
        pattern: Box<Pattern<L>>,
    },
    Sequence(Vec<Pattern<L>>),
    List {
        head: Box<Pattern<L>>,
        tail: Box<Pattern<L>>,
    },
    Set(Vec<Pattern<L>>),
    Dict(Vec<(String, Pattern<L>)>),
    Guard {
        pattern: Box<Pattern<L>>,
        predicate: L,
    },
    StringPrefix(String),
    Repeat {
        pattern: Box<Pattern<L>>,
        repeats: usize,
    },
}

/// Error raised when decoding a pattern from preserves data.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternError {
    message: String,
}

impl PatternError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PatternError {}

impl Pattern<IOValue> {
    /// Serialise the pattern into a preserves value.
    pub fn to_io_value(&self) -> IOValue {
        match self {
            Pattern::Wildcard => IOValue::record(IOValue::symbol(TAG_WILDCARD), vec![]),
            Pattern::Literal(value) => {
                IOValue::record(IOValue::symbol(TAG_LITERAL), vec![value.clone()])
            }
            Pattern::Record { label, fields } => {
                let encoded_fields: Vec<_> = fields.iter().map(|f| f.to_io_value()).collect();
                IOValue::record(
                    IOValue::symbol(TAG_RECORD),
                    std::iter::once(IOValue::symbol(label.clone()))
                        .chain(encoded_fields)
                        .collect(),
                )
            }
            Pattern::Bind { name, pattern } => IOValue::record(
                IOValue::symbol(TAG_BIND),
                vec![IOValue::symbol(name.clone()), pattern.to_io_value()],
            ),
            Pattern::Sequence(items) => IOValue::record(
                IOValue::symbol(TAG_SEQUENCE),
                items.iter().map(|item| item.to_io_value()).collect(),
            ),
            Pattern::List { head, tail } => IOValue::record(
                IOValue::symbol(TAG_LIST),
                vec![head.to_io_value(), tail.to_io_value()],
            ),
            Pattern::Set(items) => IOValue::record(
                IOValue::symbol(TAG_SET),
                items.iter().map(|item| item.to_io_value()).collect(),
            ),
            Pattern::Dict(entries) => {
                let fields = entries
                    .iter()
                    .map(|(key, pattern)| {
                        IOValue::record(
                            IOValue::symbol(TAG_DICT_ENTRY_LABEL),
                            vec![IOValue::symbol(key.clone()), pattern.to_io_value()],
                        )
                    })
                    .collect();
                IOValue::record(IOValue::symbol(TAG_DICT), fields)
            }
            Pattern::Guard { pattern, predicate } => IOValue::record(
                IOValue::symbol(TAG_GUARD),
                vec![pattern.to_io_value(), predicate.clone()],
            ),
            Pattern::StringPrefix(prefix) => IOValue::record(
                IOValue::symbol(TAG_STRING_PREFIX),
                vec![IOValue::new(prefix.clone())],
            ),
            Pattern::Repeat { pattern, repeats } => IOValue::record(
                IOValue::symbol(TAG_REPEAT),
                vec![pattern.to_io_value(), IOValue::new(*repeats as i64)],
            ),
        }
    }

    /// Decode a pattern from the `preserves` representation.
    pub fn from_io_value(value: &IOValue) -> Result<Self, PatternError> {
        if !value.is_record() {
            return Err(PatternError::new("pattern must be record"));
        }

        let label = value
            .label()
            .as_symbol()
            .map(|sym| sym.as_ref().to_string())
            .ok_or_else(|| PatternError::new("pattern label must be a symbol"))?;

        match label.as_str() {
            TAG_WILDCARD => Ok(Pattern::Wildcard),
            TAG_LITERAL => {
                if value.len() != 1 {
                    return Err(PatternError::new("literal pattern takes one field"));
                }
                Ok(Pattern::Literal(IOValue::from(value.index(0))))
            }
            TAG_RECORD => {
                if value.len() < 1 {
                    return Err(PatternError::new("record pattern missing label"));
                }
                let label_value = IOValue::from(value.index(0));
                let label_str = label_value
                    .as_symbol()
                    .map(|sym| sym.as_ref().to_string())
                    .ok_or_else(|| PatternError::new("record label must be symbol"))?;
                let mut fields = Vec::new();
                for i in 1..value.len() {
                    let field_value = IOValue::from(value.index(i));
                    fields.push(Pattern::from_io_value(&field_value)?);
                }
                Ok(Pattern::Record {
                    label: label_str,
                    fields,
                })
            }
            TAG_BIND => {
                if value.len() != 2 {
                    return Err(PatternError::new("bind pattern takes name and pattern"));
                }
                let name = value
                    .index(0)
                    .as_symbol()
                    .map(|sym| sym.as_ref().to_string())
                    .ok_or_else(|| PatternError::new("bind name must be symbol"))?;
                let subpattern = Pattern::from_io_value(&IOValue::from(value.index(1)))?;
                Ok(Pattern::Bind {
                    name,
                    pattern: Box::new(subpattern),
                })
            }
            TAG_SEQUENCE => {
                let mut items = Vec::new();
                for i in 0..value.len() {
                    items.push(Pattern::from_io_value(&IOValue::from(value.index(i)))?);
                }
                Ok(Pattern::Sequence(items))
            }
            TAG_LIST => {
                if value.len() != 2 {
                    return Err(PatternError::new("list pattern requires head and tail"));
                }
                let head = Pattern::from_io_value(&IOValue::from(value.index(0)))?;
                let tail = Pattern::from_io_value(&IOValue::from(value.index(1)))?;
                Ok(Pattern::List {
                    head: Box::new(head),
                    tail: Box::new(tail),
                })
            }
            TAG_SET => {
                let mut items = Vec::new();
                for i in 0..value.len() {
                    items.push(Pattern::from_io_value(&IOValue::from(value.index(i)))?);
                }
                Ok(Pattern::Set(items))
            }
            TAG_DICT => {
                let mut entries = Vec::new();
                for i in 0..value.len() {
                    let entry = IOValue::from(value.index(i));
                    if !entry.is_record() {
                        return Err(PatternError::new("dict entry must be record"));
                    }
                    if entry
                        .label()
                        .as_symbol()
                        .map(|sym| sym.as_ref() == TAG_DICT_ENTRY_LABEL)
                        != Some(true)
                    {
                        return Err(PatternError::new(
                            "dict entry record must use :pattern/dict-entry label",
                        ));
                    }
                    if entry.len() != 2 {
                        return Err(PatternError::new("dict entry must include key and pattern"));
                    }
                    let first_field = IOValue::from(entry.index(0));
                    let key = first_field
                        .as_symbol()
                        .map(|sym| sym.as_ref().to_string())
                        .or_else(|| first_field.as_string().map(|s| s.to_string()))
                        .ok_or_else(|| PatternError::new("dict keys must be symbol or string"))?;
                    let pattern = Pattern::from_io_value(&IOValue::from(entry.index(1)))?;
                    entries.push((key, pattern));
                }
                Ok(Pattern::Dict(entries))
            }
            TAG_GUARD => {
                if value.len() != 2 {
                    return Err(PatternError::new(
                        "guard pattern takes pattern and predicate",
                    ));
                }
                let pattern = Pattern::from_io_value(&IOValue::from(value.index(0)))?;
                Ok(Pattern::Guard {
                    pattern: Box::new(pattern),
                    predicate: IOValue::from(value.index(1)),
                })
            }
            TAG_STRING_PREFIX => {
                if value.len() != 1 {
                    return Err(PatternError::new(
                        "string-prefix pattern takes exactly one argument",
                    ));
                }
                let first_field = IOValue::from(value.index(0));
                let prefix = first_field
                    .as_string()
                    .map(|s| s.to_string())
                    .ok_or_else(|| PatternError::new("string-prefix expects string"))?;
                Ok(Pattern::StringPrefix(prefix))
            }
            TAG_REPEAT => {
                if value.len() != 2 {
                    return Err(PatternError::new(
                        "repeat pattern expects pattern and repeat count",
                    ));
                }
                let pattern = Pattern::from_io_value(&IOValue::from(value.index(0)))?;
                let count_value = IOValue::from(value.index(1));
                let integer = count_value
                    .as_signed_integer()
                    .ok_or_else(|| PatternError::new("repeat count must be integer"))?;
                let repeats = i64::try_from(integer.as_ref())
                    .map_err(|_| PatternError::new("repeat count out of range"))?;
                if repeats <= 0 {
                    return Err(PatternError::new("repeat count must be a positive integer"));
                }
                Ok(Pattern::Repeat {
                    pattern: Box::new(pattern),
                    repeats: repeats as usize,
                })
            }
            other => Err(PatternError::new(format!(
                "unknown pattern label: {}",
                other
            ))),
        }
    }
}

impl<L: LiteralLabel> Pattern<L> {
    /// Attempt to derive a human-friendly label hint from this pattern.
    pub fn label_hint(&self) -> Option<String> {
        match self {
            Pattern::Literal(value) => value.literal_hint(),
            Pattern::Record { label, fields }
                if (label == "workspace/read" || label == "workspace/write")
                    && !fields.is_empty() =>
            {
                match &fields[0] {
                    Pattern::Literal(value) => value.literal_string(),
                    Pattern::StringPrefix(prefix) => Some(prefix.clone()),
                    Pattern::Bind { pattern, .. } => pattern.label_hint(),
                    _ => None,
                }
            }
            Pattern::Record { .. } => None,
            Pattern::Bind { pattern, .. } => pattern.label_hint(),
            Pattern::Sequence(items) | Pattern::Set(items) => {
                items.iter().find_map(|pattern| pattern.label_hint())
            }
            Pattern::Dict(entries) => entries.iter().find_map(|(_, p)| p.label_hint()),
            Pattern::Guard { pattern, .. } => pattern.label_hint(),
            Pattern::StringPrefix(prefix) => Some(prefix.clone()),
            Pattern::List { head, tail } => head.label_hint().or_else(|| tail.label_hint()),
            Pattern::Repeat { pattern, .. } => pattern.label_hint(),
            Pattern::Wildcard => None,
        }
    }
}

impl<L: Clone> Pattern<L> {
    fn to_tree_pattern(&self) -> TreePattern<L> {
        match self {
            Pattern::Repeat { pattern, repeats } => {
                let mut inner = pattern.to_tree_pattern();
                inner.repeats = *repeats;
                inner
            }
            pattern => {
                let kind = match pattern {
                    Pattern::Wildcard => TreePatternKind::Wildcard,
                    Pattern::Literal(value) => TreePatternKind::Literal(value.clone()),
                    Pattern::Record { label, fields } => TreePatternKind::Record {
                        label: label.clone(),
                        fields: fields.iter().map(|f| f.to_tree_pattern()).collect(),
                    },
                    Pattern::Bind { name, pattern } => TreePatternKind::Binding {
                        name: name.clone(),
                        pattern: Some(Box::new(pattern.to_tree_pattern())),
                    },
                    Pattern::Sequence(items) => TreePatternKind::Vector {
                        elements: items.iter().map(|item| item.to_tree_pattern()).collect(),
                    },
                    Pattern::List { head, tail } => TreePatternKind::List {
                        elements: vec![head.to_tree_pattern()],
                        tail: Some(Box::new(tail.to_tree_pattern())),
                    },
                    Pattern::Set(items) => TreePatternKind::Set {
                        elements: items.iter().map(|item| item.to_tree_pattern()).collect(),
                    },
                    Pattern::Dict(entries) => TreePatternKind::Dict {
                        entries: entries
                            .iter()
                            .map(|(key, pattern)| (key.clone(), pattern.to_tree_pattern()))
                            .collect(),
                    },
                    Pattern::Guard { pattern, .. } => TreePatternKind::Guard {
                        pattern: Box::new(pattern.to_tree_pattern()),
                    },
                    Pattern::StringPrefix(prefix) => TreePatternKind::StringPrefix(prefix.clone()),
                    Pattern::Repeat { .. } => unreachable!(),
                };

                TreePattern { kind, repeats: 0 }
            }
        }
    }

    /// Check if the pattern matches a value that implements [`MatchDatum`].
    pub fn matches_tagged<D>(&self, value: &D) -> bool
    where
        D: MatchDatum<L>,
    {
        let tree_pattern = self.to_tree_pattern();
        match_clause(
            std::slice::from_ref(&tree_pattern),
            None,
            std::slice::from_ref(value),
        )
        .is_some()
    }

    /// Attempt to match the pattern and capture bindings as [`tree::BindingValue`]s.
    pub fn match_tagged_with_bindings<D>(
        &self,
        value: &D,
    ) -> Option<HashMap<String, BindingValue<D>>>
    where
        D: MatchDatum<L>,
    {
        let tree_pattern = self.to_tree_pattern();
        let match_result = match_clause(
            std::slice::from_ref(&tree_pattern),
            None,
            std::slice::from_ref(value),
        )?;

        Some(match_result)
    }

    /// Convert the literal payloads of this pattern using the provided mapper.
    pub fn map_literals<M, E, F>(&self, f: &mut F) -> Result<Pattern<M>, E>
    where
        F: FnMut(&L) -> Result<M, E>,
    {
        Ok(match self {
            Pattern::Wildcard => Pattern::Wildcard,
            Pattern::Literal(value) => Pattern::Literal(f(value)?),
            Pattern::Record { label, fields } => Pattern::Record {
                label: label.clone(),
                fields: fields
                    .iter()
                    .map(|field| field.map_literals(f))
                    .collect::<Result<_, E>>()?,
            },
            Pattern::Bind { name, pattern } => Pattern::Bind {
                name: name.clone(),
                pattern: Box::new(pattern.map_literals(f)?),
            },
            Pattern::Sequence(items) => Pattern::Sequence(
                items
                    .iter()
                    .map(|item| item.map_literals(f))
                    .collect::<Result<_, E>>()?,
            ),
            Pattern::List { head, tail } => Pattern::List {
                head: Box::new(head.map_literals(f)?),
                tail: Box::new(tail.map_literals(f)?),
            },
            Pattern::Set(items) => Pattern::Set(
                items
                    .iter()
                    .map(|item| item.map_literals(f))
                    .collect::<Result<_, E>>()?,
            ),
            Pattern::Dict(entries) => Pattern::Dict(
                entries
                    .iter()
                    .map(|(key, pattern)| Ok((key.clone(), pattern.map_literals(f)?)))
                    .collect::<Result<_, E>>()?,
            ),
            Pattern::Guard { pattern, predicate } => Pattern::Guard {
                pattern: Box::new(pattern.map_literals(f)?),
                predicate: f(predicate)?,
            },
            Pattern::StringPrefix(prefix) => Pattern::StringPrefix(prefix.clone()),
            Pattern::Repeat { pattern, repeats } => Pattern::Repeat {
                pattern: Box::new(pattern.map_literals(f)?),
                repeats: *repeats,
            },
        })
    }
}

/// Helper for building patterns from code or tests.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternBuilder<L = IOValue>(PhantomData<fn() -> L>);

impl<L> PatternBuilder<L> {
    pub fn wildcard() -> Pattern<L> {
        Pattern::Wildcard
    }

    pub fn literal(value: L) -> Pattern<L> {
        Pattern::Literal(value)
    }

    pub fn record(label: impl Into<String>, fields: Vec<Pattern<L>>) -> Pattern<L> {
        Pattern::Record {
            label: label.into(),
            fields,
        }
    }

    pub fn bind(name: impl Into<String>, pattern: Pattern<L>) -> Pattern<L> {
        Pattern::Bind {
            name: name.into(),
            pattern: Box::new(pattern),
        }
    }

    pub fn sequence(items: Vec<Pattern<L>>) -> Pattern<L> {
        Pattern::Sequence(items)
    }

    pub fn list(head: Pattern<L>, tail: Pattern<L>) -> Pattern<L> {
        Pattern::List {
            head: Box::new(head),
            tail: Box::new(tail),
        }
    }

    pub fn set(items: Vec<Pattern<L>>) -> Pattern<L> {
        Pattern::Set(items)
    }

    pub fn dict(entries: Vec<(String, Pattern<L>)>) -> Pattern<L> {
        Pattern::Dict(entries)
    }

    pub fn guard(pattern: Pattern<L>, predicate: L) -> Pattern<L> {
        Pattern::Guard {
            pattern: Box::new(pattern),
            predicate,
        }
    }

    pub fn string_prefix(prefix: impl Into<String>) -> Pattern<L> {
        Pattern::StringPrefix(prefix.into())
    }

    pub fn repeat(pattern: Pattern<L>, repeats: usize) -> Pattern<L> {
        Pattern::Repeat {
            pattern: Box::new(pattern),
            repeats: repeats.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl LiteralLabel for String {
        fn literal_string(&self) -> Option<String> {
            Some(self.clone())
        }
    }

    #[test]
    fn record_fields_are_visible_through_match_datum() {
        let record = IOValue::record(
            IOValue::symbol("foo"),
            vec![IOValue::new(1i64), IOValue::new(2i64)],
        );
        let (label, fields) = record
            .record_fields()
            .expect("record exposes label and fields");
        assert_eq!(label, "foo");
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn pattern_roundtrip_record() {
        let pattern = PatternBuilder::record(
            "Foo",
            vec![
                PatternBuilder::bind("x", PatternBuilder::wildcard()),
                PatternBuilder::literal(IOValue::new(5i64)),
            ],
        );

        let encoded = pattern.to_io_value();
        assert_eq!(
            encoded
                .label()
                .as_symbol()
                .map(|sym| sym.as_ref().to_string()),
            Some(TAG_RECORD.to_string())
        );

        let decoded = Pattern::from_io_value(&encoded).expect("roundtrip record pattern");
        assert_eq!(decoded, pattern);
    }

    #[test]
    fn match_tagged_with_bindings_returns_binding_values() {
        let pattern = PatternBuilder::record(
            "point",
            vec![
                PatternBuilder::bind("x", PatternBuilder::wildcard()),
                PatternBuilder::bind("y", PatternBuilder::wildcard()),
            ],
        );

        let value = IOValue::record(
            IOValue::symbol("point"),
            vec![IOValue::new(10i64), IOValue::new(20i64)],
        );

        let bindings = pattern
            .match_tagged_with_bindings(&value)
            .expect("pattern matches");
        assert!(matches!(
            bindings.get("x"),
            Some(BindingValue::Leaf(v)) if *v == IOValue::new(10i64)
        ));
        assert!(matches!(
            bindings.get("y"),
            Some(BindingValue::Leaf(v)) if *v == IOValue::new(20i64)
        ));
    }

    #[test]
    fn literal_label_hint_uses_trait_based_strings() {
        let pattern = PatternBuilder::<String>::literal("hello".to_string());
        assert_eq!(pattern.label_hint().as_deref(), Some("hello"));
    }
}
