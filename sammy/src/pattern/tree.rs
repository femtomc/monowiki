//! Generic tree-pattern matcher that supports nested ellipses.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Describes how to view a datum for pattern matching.
pub trait MatchDatum<L>: Clone + PartialEq {
    /// Return true if the datum matches the literal.
    fn literal_matches(&self, literal: &L) -> bool;

    /// If the datum is a list, return its children plus optional dotted tail.
    fn list_children(&self) -> Option<(Vec<Self>, Option<Self>)>;

    /// If the datum is a vector, return its children.
    fn vector_children(&self) -> Option<Vec<Self>>;

    /// If the datum is a record, return the label and fields.
    fn record_fields(&self) -> Option<(String, Vec<Self>)> {
        None
    }

    /// If the datum represents a set, return its elements.
    fn set_items(&self) -> Option<Vec<Self>> {
        None
    }

    /// If the datum represents a dictionary, return key/value entries.
    fn dict_entries(&self) -> Option<Vec<(String, Self)>> {
        None
    }

    /// If the datum can be treated as text for prefix checks, return it.
    fn string_value(&self) -> Option<String> {
        None
    }
}

/// Canonical pattern tree that supports ellipsis repetition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreePattern<L> {
    pub kind: TreePatternKind<L>,
    pub repeats: usize,
}

/// Pattern node kinds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TreePatternKind<L> {
    Literal(L),
    Binding {
        name: String,
        pattern: Option<Box<TreePattern<L>>>,
    },
    Wildcard,
    List {
        elements: Vec<TreePattern<L>>,
        tail: Option<Box<TreePattern<L>>>,
    },
    Vector {
        elements: Vec<TreePattern<L>>,
    },
    Record {
        label: String,
        fields: Vec<TreePattern<L>>,
    },
    Set {
        elements: Vec<TreePattern<L>>,
    },
    Dict {
        entries: Vec<(String, TreePattern<L>)>,
    },
    Guard {
        pattern: Box<TreePattern<L>>,
    },
    StringPrefix(String),
}

/// Hierarchical binding captured during matching.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingValue<D> {
    Leaf(D),
    Sequence(Vec<BindingValue<D>>),
}

impl<D> BindingValue<D> {
    pub fn leaf(value: D) -> Self {
        BindingValue::Leaf(value)
    }

    pub fn sequence(items: Vec<BindingValue<D>>) -> Self {
        BindingValue::Sequence(items)
    }
}

#[derive(Debug, Clone)]
struct BindingEnv<D> {
    bindings: HashMap<String, BindingValue<D>>,
}

impl<D> Default for BindingEnv<D> {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }
}

impl<D: Clone + PartialEq> BindingEnv<D> {
    fn bind_value(&mut self, name: &str, value: D, path: &[usize]) -> bool {
        match self.bindings.get_mut(name) {
            Some(existing) => insert_at(existing, path, BindingValue::Leaf(value)),
            None => {
                let binding = BindingValue::Leaf(value);
                let shaped = shape_from_path(binding, path);
                self.bindings.insert(name.to_string(), shaped);
                true
            }
        }
    }

    fn bind_rest(&mut self, name: &str, values: Vec<D>) -> bool {
        let binding = BindingValue::Sequence(
            values
                .into_iter()
                .map(BindingValue::Leaf)
                .collect::<Vec<_>>(),
        );
        match self.bindings.get(name) {
            Some(existing) => existing == &binding,
            None => {
                self.bindings.insert(name.to_string(), binding);
                true
            }
        }
    }

    fn ensure_sequence_binding(&mut self, name: &str) {
        self.bindings
            .entry(name.to_string())
            .or_insert_with(|| BindingValue::Sequence(Vec::new()));
    }

    fn ensure_path(&mut self, name: &str, path: &[usize]) -> bool {
        let entry = self
            .bindings
            .entry(name.to_string())
            .or_insert_with(|| BindingValue::Sequence(Vec::new()));
        ensure_path(entry, path)
    }

    fn into_bindings(self) -> HashMap<String, BindingValue<D>> {
        self.bindings
    }
}

fn shape_from_path<D>(value: BindingValue<D>, path: &[usize]) -> BindingValue<D> {
    if path.is_empty() {
        return value;
    }
    let idx = path[0];
    let mut children = Vec::with_capacity(idx + 1);
    for _ in 0..idx {
        children.push(BindingValue::Sequence(Vec::new()));
    }
    children.push(shape_from_path(value, &path[1..]));
    BindingValue::Sequence(children)
}

fn insert_at<D: Clone + PartialEq>(
    binding: &mut BindingValue<D>,
    path: &[usize],
    value: BindingValue<D>,
) -> bool {
    if path.is_empty() {
        match binding {
            BindingValue::Leaf(existing) => {
                if let BindingValue::Leaf(new_value) = value {
                    *existing == new_value
                } else {
                    false
                }
            }
            BindingValue::Sequence(_) => false,
        }
    } else {
        match binding {
            BindingValue::Leaf(_) => false,
            BindingValue::Sequence(children) => {
                let idx = path[0];
                while children.len() <= idx {
                    children.push(BindingValue::Sequence(Vec::new()));
                }
                if path.len() == 1 {
                    match &mut children[idx] {
                        BindingValue::Leaf(existing) => {
                            if let BindingValue::Leaf(new_value) = value {
                                *existing == new_value
                            } else {
                                false
                            }
                        }
                        BindingValue::Sequence(inner) => {
                            if inner.is_empty() {
                                children[idx] = value;
                                true
                            } else {
                                false
                            }
                        }
                    }
                } else {
                    insert_at(&mut children[idx], &path[1..], value)
                }
            }
        }
    }
}

fn ensure_path<D>(binding: &mut BindingValue<D>, path: &[usize]) -> bool {
    if path.is_empty() {
        return true;
    }
    match binding {
        BindingValue::Leaf(_) => false,
        BindingValue::Sequence(children) => {
            let idx = path[0];
            while children.len() <= idx {
                children.push(BindingValue::Sequence(Vec::new()));
            }
            ensure_path(&mut children[idx], &path[1..])
        }
    }
}

/// Attempt to match a clause against provided arguments.
pub fn match_clause<L, D>(
    patterns: &[TreePattern<L>],
    rest: Option<&str>,
    args: &[D],
) -> Option<HashMap<String, BindingValue<D>>>
where
    L: Clone,
    D: MatchDatum<L>,
{
    let mut env = BindingEnv::default();
    let binding_shapes = clause_binding_depths(patterns);
    for (name, depth) in binding_shapes.iter() {
        if *depth > 0 {
            env.ensure_sequence_binding(name);
        }
    }

    let mut path = Vec::new();
    if let Some(consumed) =
        match_sequence_prefix(patterns, args, &mut env, &mut path, &binding_shapes)
    {
        let remainder = &args[consumed..];
        if let Some(rest_name) = rest {
            if env.bind_rest(rest_name, remainder.to_vec()) {
                return Some(env.into_bindings());
            }
        } else if remainder.is_empty() {
            return Some(env.into_bindings());
        }
    }

    None
}

fn match_sequence_prefix<L, D>(
    patterns: &[TreePattern<L>],
    values: &[D],
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> Option<usize>
where
    L: Clone,
    D: MatchDatum<L>,
{
    if patterns.is_empty() {
        return Some(0);
    }

    let pattern = &patterns[0];
    if pattern.repeats == 0 {
        if values.is_empty() {
            return None;
        }
        let mut snapshot = env.clone();
        if match_pattern(pattern, &values[0], &mut snapshot, path, shapes) {
            if let Some(consumed_rest) =
                match_sequence_prefix(&patterns[1..], &values[1..], &mut snapshot, path, shapes)
            {
                *env = snapshot;
                return Some(consumed_rest + 1);
            }
        }
        None
    } else {
        for count in (0..=values.len()).rev() {
            let mut snapshot = env.clone();
            if match_repeated(pattern, &values[..count], &mut snapshot, path, shapes) {
                if let Some(consumed_rest) = match_sequence_prefix(
                    &patterns[1..],
                    &values[count..],
                    &mut snapshot,
                    path,
                    shapes,
                ) {
                    *env = snapshot;
                    return Some(consumed_rest + count);
                }
            }
        }
        None
    }
}

fn match_pattern<L, D>(
    pattern: &TreePattern<L>,
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    if pattern.repeats > 0 {
        return match_repeated(pattern, std::slice::from_ref(value), env, path, shapes);
    }

    match &pattern.kind {
        TreePatternKind::Literal(expected) => value.literal_matches(expected),
        TreePatternKind::Binding {
            name,
            pattern: inner,
        } => {
            if let Some(inner_pattern) = inner {
                if !match_pattern(inner_pattern, value, env, path, shapes) {
                    return false;
                }
            }
            env.bind_value(name, value.clone(), path)
        }
        TreePatternKind::Wildcard => true,
        TreePatternKind::List { elements, tail } => {
            match_list_pattern(elements, tail, value, env, path, shapes)
        }
        TreePatternKind::Vector { elements } => {
            match_vector_pattern(elements, value, env, path, shapes)
        }
        TreePatternKind::Record { label, fields } => {
            match_record_pattern(label, fields, value, env, path, shapes)
        }
        TreePatternKind::Set { elements } => match_set_pattern(elements, value, env, path, shapes),
        TreePatternKind::Dict { entries } => match_dict_pattern(entries, value, env, path, shapes),
        TreePatternKind::Guard { pattern } => match_pattern(pattern, value, env, path, shapes),
        TreePatternKind::StringPrefix(prefix) => match_string_prefix_pattern(prefix, value),
    }
}

fn match_repeated<L, D>(
    pattern: &TreePattern<L>,
    values: &[D],
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    if values.is_empty() {
        return true;
    }

    let mut snapshot = env.clone();
    let mut base = pattern.clone();
    base.repeats = 0;
    let binding_names = pattern_binding_names(&base);

    path.push(0);
    for (idx, value) in values.iter().enumerate() {
        if let Some(last) = path.last_mut() {
            *last = idx;
        }
        if !ensure_placeholders(&mut snapshot, &binding_names, path.as_slice(), shapes) {
            path.pop();
            return false;
        }
        let mut inner = snapshot.clone();
        if !match_pattern(&base, value, &mut inner, path, shapes) {
            path.pop();
            return false;
        }
        snapshot = inner;
    }
    path.pop();

    *env = snapshot;
    true
}

fn match_list_pattern<L, D>(
    elements: &[TreePattern<L>],
    tail: &Option<Box<TreePattern<L>>>,
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    let (children, tail_value) = match value.list_children() {
        Some(data) => data,
        None => return false,
    };

    let mut snapshot = env.clone();
    if !match_sequence_exact(elements, &children, &mut snapshot, path, shapes) {
        return false;
    }

    match (tail, tail_value) {
        (Some(pattern), Some(tail_datum)) => {
            if match_pattern(pattern, &tail_datum, &mut snapshot, path, shapes) {
                *env = snapshot;
                true
            } else {
                false
            }
        }
        (None, None) => {
            *env = snapshot;
            true
        }
        _ => false,
    }
}

fn match_vector_pattern<L, D>(
    elements: &[TreePattern<L>],
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    let children = match value.vector_children() {
        Some(items) => items,
        None => return false,
    };

    match_sequence_exact(elements, &children, env, path, shapes)
}

fn match_record_pattern<L, D>(
    label: &str,
    fields: &[TreePattern<L>],
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    let (record_label, record_fields) = match value.record_fields() {
        Some(data) => data,
        None => return false,
    };

    if record_label != label || record_fields.len() != fields.len() {
        return false;
    }

    let mut snapshot = env.clone();
    if !match_sequence_exact(fields, &record_fields, &mut snapshot, path, shapes) {
        return false;
    }
    *env = snapshot;
    true
}

fn match_set_pattern<L, D>(
    elements: &[TreePattern<L>],
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    let items = match value.set_items() {
        Some(items) => items,
        None => return false,
    };

    let mut snapshot = env.clone();
    if match_set_elements(elements, items, &mut snapshot, path, shapes) {
        *env = snapshot;
        true
    } else {
        false
    }
}

fn match_set_elements<L, D>(
    elements: &[TreePattern<L>],
    items: Vec<D>,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    if elements.is_empty() {
        return true;
    }

    for idx in 0..items.len() {
        let mut remaining = items.clone();
        let candidate = remaining.remove(idx);
        let mut snapshot = env.clone();
        if match_pattern(&elements[0], &candidate, &mut snapshot, path, shapes) {
            if match_set_elements(&elements[1..], remaining, &mut snapshot, path, shapes) {
                *env = snapshot;
                return true;
            }
        }
    }

    false
}

fn match_dict_pattern<L, D>(
    entries: &[(String, TreePattern<L>)],
    value: &D,
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    let mut values = match value.dict_entries() {
        Some(entries) => entries,
        None => return false,
    };

    let mut snapshot = env.clone();
    for (key, pattern) in entries {
        let position = values
            .iter()
            .position(|(candidate_key, _)| candidate_key == key);
        let idx = match position {
            Some(idx) => idx,
            None => return false,
        };
        let candidate = values.remove(idx).1;
        let mut inner = snapshot.clone();
        if !match_pattern(pattern, &candidate, &mut inner, path, shapes) {
            return false;
        }
        snapshot = inner;
    }

    *env = snapshot;
    true
}

fn match_string_prefix_pattern<L, D>(prefix: &str, value: &D) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    match value.string_value() {
        Some(text) => text.starts_with(prefix),
        None => false,
    }
}

fn match_sequence_exact<L, D>(
    patterns: &[TreePattern<L>],
    values: &[D],
    env: &mut BindingEnv<D>,
    path: &mut Vec<usize>,
    shapes: &HashMap<String, usize>,
) -> bool
where
    L: Clone,
    D: MatchDatum<L>,
{
    match_sequence_prefix(patterns, values, env, path, shapes)
        .map(|consumed| consumed == values.len())
        .unwrap_or(false)
}

fn ensure_placeholders<D>(
    env: &mut BindingEnv<D>,
    names: &[String],
    path: &[usize],
    shapes: &HashMap<String, usize>,
) -> bool
where
    D: Clone + PartialEq,
{
    let current_depth = path.len();
    for name in names {
        let expected_depth = *shapes.get(name).unwrap_or(&0);
        if expected_depth > current_depth && !env.ensure_path(name, path) {
            return false;
        }
    }
    true
}

fn clause_binding_depths<L>(patterns: &[TreePattern<L>]) -> HashMap<String, usize> {
    let mut shapes = HashMap::new();
    for pattern in patterns {
        collect_binding_depths(pattern, 0, &mut shapes);
    }
    shapes
}

fn collect_binding_depths<L>(
    pattern: &TreePattern<L>,
    depth: usize,
    shapes: &mut HashMap<String, usize>,
) {
    let nested_depth = depth + pattern.repeats;
    match &pattern.kind {
        TreePatternKind::Binding { name, pattern } => {
            shapes
                .entry(name.clone())
                .and_modify(|existing| {
                    if *existing < nested_depth {
                        *existing = nested_depth;
                    }
                })
                .or_insert(nested_depth);
            if let Some(inner) = pattern {
                collect_binding_depths(inner, nested_depth, shapes);
            }
        }
        TreePatternKind::List { elements, tail } => {
            for element in elements {
                collect_binding_depths(element, nested_depth, shapes);
            }
            if let Some(tail_pattern) = tail {
                collect_binding_depths(tail_pattern, nested_depth, shapes);
            }
        }
        TreePatternKind::Vector { elements } => {
            for element in elements {
                collect_binding_depths(element, nested_depth, shapes);
            }
        }
        TreePatternKind::Record { fields, .. } => {
            for field in fields {
                collect_binding_depths(field, nested_depth, shapes);
            }
        }
        TreePatternKind::Set { elements } => {
            for element in elements {
                collect_binding_depths(element, nested_depth, shapes);
            }
        }
        TreePatternKind::Dict { entries } => {
            for (_, pattern) in entries {
                collect_binding_depths(pattern, nested_depth, shapes);
            }
        }
        TreePatternKind::Guard { pattern } => {
            collect_binding_depths(pattern, nested_depth, shapes);
        }
        TreePatternKind::StringPrefix(_) => {}
        TreePatternKind::Wildcard => {}
        TreePatternKind::Literal(_) => {}
    }
}

fn pattern_binding_names<L>(pattern: &TreePattern<L>) -> Vec<String> {
    let mut names = HashSet::new();
    collect_pattern_names(pattern, &mut names);
    names.into_iter().collect()
}

fn collect_pattern_names<L>(pattern: &TreePattern<L>, names: &mut HashSet<String>) {
    match &pattern.kind {
        TreePatternKind::Binding { name, pattern } => {
            names.insert(name.clone());
            if let Some(inner) = pattern {
                collect_pattern_names(inner, names);
            }
        }
        TreePatternKind::List { elements, tail } => {
            for element in elements {
                collect_pattern_names(element, names);
            }
            if let Some(tail_pattern) = tail {
                collect_pattern_names(tail_pattern, names);
            }
        }
        TreePatternKind::Vector { elements } => {
            for element in elements {
                collect_pattern_names(element, names);
            }
        }
        TreePatternKind::Record { fields, .. } => {
            for field in fields {
                collect_pattern_names(field, names);
            }
        }
        TreePatternKind::Set { elements } => {
            for element in elements {
                collect_pattern_names(element, names);
            }
        }
        TreePatternKind::Dict { entries } => {
            for (_, pattern) in entries {
                collect_pattern_names(pattern, names);
            }
        }
        TreePatternKind::Guard { pattern } => collect_pattern_names(pattern, names),
        TreePatternKind::StringPrefix(_) => {}
        TreePatternKind::Wildcard => {}
        TreePatternKind::Literal(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct TestDatum(Value);

    #[derive(Clone, Debug, PartialEq)]
    enum Value {
        Literal(&'static str),
        List(Vec<Value>),
        Vector(Vec<Value>),
        Record(&'static str, Vec<Value>),
        Set(Vec<Value>),
        Dict(Vec<(&'static str, Value)>),
        Text(&'static str),
    }

    impl MatchDatum<&'static str> for TestDatum {
        fn literal_matches(&self, literal: &&'static str) -> bool {
            matches!(&self.0, Value::Literal(text) if text == literal)
        }

        fn list_children(&self) -> Option<(Vec<Self>, Option<Self>)> {
            match &self.0 {
                Value::List(items) => Some((items.iter().cloned().map(TestDatum).collect(), None)),
                _ => None,
            }
        }

        fn vector_children(&self) -> Option<Vec<Self>> {
            match &self.0 {
                Value::Vector(items) => Some(items.iter().cloned().map(TestDatum).collect()),
                _ => None,
            }
        }

        fn record_fields(&self) -> Option<(String, Vec<Self>)> {
            match &self.0 {
                Value::Record(label, fields) => Some((
                    (*label).to_string(),
                    fields.iter().cloned().map(TestDatum).collect(),
                )),
                _ => None,
            }
        }

        fn set_items(&self) -> Option<Vec<Self>> {
            match &self.0 {
                Value::Set(items) => Some(items.iter().cloned().map(TestDatum).collect()),
                _ => None,
            }
        }

        fn dict_entries(&self) -> Option<Vec<(String, Self)>> {
            match &self.0 {
                Value::Dict(entries) => Some(
                    entries
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), TestDatum(v.clone())))
                        .collect(),
                ),
                _ => None,
            }
        }

        fn string_value(&self) -> Option<String> {
            match &self.0 {
                Value::Text(text) => Some((*text).to_string()),
                _ => None,
            }
        }
    }

    fn binding(name: &str, repeats: usize) -> TreePattern<&'static str> {
        TreePattern {
            kind: TreePatternKind::Binding {
                name: name.to_string(),
                pattern: None,
            },
            repeats,
        }
    }

    fn list(patterns: Vec<TreePattern<&'static str>>, repeats: usize) -> TreePattern<&'static str> {
        TreePattern {
            kind: TreePatternKind::List {
                elements: patterns,
                tail: None,
            },
            repeats,
        }
    }

    #[test]
    fn nested_ellipses_capture_structure() {
        let inner = list(vec![binding("x", 1)], 0);
        let mut repeated_inner = inner.clone();
        repeated_inner.repeats = 1;
        let outer = list(vec![repeated_inner], 0);

        let datum = TestDatum(Value::List(vec![
            Value::List(vec![Value::Literal("a"), Value::Literal("b")]),
            Value::List(vec![Value::Literal("c")]),
        ]));

        let bindings = match_clause(&[outer], None, &[datum]).expect("pattern matches");
        let x_binding = bindings.get("x").expect("binding x");
        match x_binding {
            BindingValue::Sequence(groups) => {
                assert_eq!(groups.len(), 2);
                match &groups[0] {
                    BindingValue::Sequence(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner[0], BindingValue::Leaf(TestDatum(Value::Literal("a"))));
                        assert_eq!(inner[1], BindingValue::Leaf(TestDatum(Value::Literal("b"))));
                    }
                    other => panic!("unexpected binding {:?}", other),
                }
                match &groups[1] {
                    BindingValue::Sequence(inner) => {
                        assert_eq!(inner.len(), 1);
                        assert_eq!(inner[0], BindingValue::Leaf(TestDatum(Value::Literal("c"))));
                    }
                    other => panic!("unexpected binding {:?}", other),
                }
            }
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn record_patterns_match_fields() {
        let pattern = TreePattern {
            kind: TreePatternKind::Record {
                label: "point".to_string(),
                fields: vec![binding("x", 0), binding("y", 0)],
            },
            repeats: 0,
        };

        let datum = TestDatum(Value::Record(
            "point",
            vec![Value::Literal("x1"), Value::Literal("y1")],
        ));

        let bindings = match_clause(&[pattern], None, &[datum]).expect("record matches");
        assert!(bindings.contains_key("x"));
        assert!(bindings.contains_key("y"));
    }

    #[test]
    fn set_patterns_ignore_order() {
        let pattern = TreePattern {
            kind: TreePatternKind::Set {
                elements: vec![binding("a", 0), binding("b", 0)],
            },
            repeats: 0,
        };

        let datum = TestDatum(Value::Set(vec![
            Value::Literal("first"),
            Value::Literal("second"),
        ]));

        let bindings = match_clause(&[pattern], None, &[datum]).expect("set matches");
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn dict_patterns_match_by_key() {
        let pattern = TreePattern {
            kind: TreePatternKind::Dict {
                entries: vec![
                    ("left".to_string(), binding("l", 0)),
                    ("right".to_string(), binding("r", 0)),
                ],
            },
            repeats: 0,
        };

        let datum = TestDatum(Value::Dict(vec![
            ("right", Value::Literal("R")),
            ("left", Value::Literal("L")),
        ]));

        let bindings = match_clause(&[pattern], None, &[datum]).expect("dict matches");
        assert!(bindings.contains_key("l"));
        assert!(bindings.contains_key("r"));
    }

    #[test]
    fn string_prefix_patterns_match_text() {
        let pattern = TreePattern {
            kind: TreePatternKind::StringPrefix("pre".to_string()),
            repeats: 0,
        };

        let datum = TestDatum(Value::Text("prefix-value"));
        assert!(
            match_clause(&[pattern], None, &[datum]).is_some(),
            "string prefix should match"
        );
    }
}
