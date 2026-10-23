//! Conservative expansion accounting before the engine eagerly collects errors.
//!
//! Memoization stores an expanded cost, not a visited bit: two references to a
//! cached subtree still pay its cost twice. No predicates or patterns are run.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::{guard, validate::Draft};

const MAX_WORK: usize = 100_000;
const MAX_STACK: usize = 256;
const MAX_ERROR_BYTES: usize = 16 * 1024 * 1024;
const MAX_INDEX: usize = 4_096;

fn address(value: &Value) -> usize {
    value as *const Value as usize
}

fn budget() -> PluginError {
    PluginError::new("schema.work_budget").with("limit", MAX_WORK as u64)
}

fn reference_error(path: &str, detail: &str) -> PluginError {
    PluginError::new("schema.reference_target")
        .with("path", path)
        .with("detail", detail)
}

#[derive(Clone)]
struct Node<'a> {
    value: &'a Value,
    resource: &'a Value,
    path: String,
}

struct Reference {
    source: usize,
    keyword: &'static str,
    text: String,
}

struct Index<'a> {
    nodes: HashMap<usize, Node<'a>>,
    anchors: HashMap<(usize, String), usize>,
    dynamic: HashMap<String, Vec<usize>>,
    recursive: Vec<usize>,
    references: Vec<Reference>,
    edges: HashMap<(usize, &'static str), Vec<usize>>,
    roots: Vec<(&'a Value, String)>,
    ids: HashMap<String, usize>,
    draft: Draft,
}

impl<'a> Index<'a> {
    fn build(root: &'a Value, draft: Draft) -> PluginResult<Self> {
        let mut index = Self {
            nodes: HashMap::new(),
            anchors: HashMap::new(),
            dynamic: HashMap::new(),
            recursive: Vec::new(),
            references: Vec::new(),
            edges: HashMap::new(),
            roots: vec![(root, String::new())],
            ids: HashMap::new(),
            draft,
        };
        index.register(root, root, String::new())?;
        let mut position = 0;
        // A pointer into an annotation/extension can introduce another active
        // schema tree. Register it and inspect its references before compiling.
        while position < index.references.len() {
            let reference = &index.references[position];
            let source = reference.source;
            let keyword = reference.keyword;
            let text = reference.text.clone();
            let (target, resource, path) = index.resolve(source, &text)?;
            if !target.is_object() && !target.is_boolean() {
                return Err(reference_error(&path, "Reference target is not a schema"));
            }
            index.roots.push((target, path.clone()));
            index.register(target, resource, path)?;
            index.edges.insert((source, keyword), vec![address(target)]);
            position += 1;
        }
        // Dynamic resolution may choose an outer resource's matching anchor.
        // Counting every matching candidate is conservative without pretending
        // to execute the engine's dynamic-scope stack.
        for reference in &index.references {
            let targets = index
                .edges
                .get_mut(&(reference.source, reference.keyword))
                .expect("resolved reference");
            if reference.keyword == "$dynamicRef" {
                let fragment = decode_fragment(&reference.text[1..])
                    .map_err(|detail| reference_error("", detail))?;
                if !fragment.starts_with('/')
                    && let Some(candidates) = index.dynamic.get(&fragment)
                {
                    targets.extend(candidates);
                }
            } else if reference.keyword == "$recursiveRef" {
                targets.extend(&index.recursive);
            }
            targets.sort_unstable();
            targets.dedup();
        }
        guard::schema_many(&index.roots, draft).map_err(|issue| {
            reference_error(&issue.path, &format!("{}: {}", issue.code, issue.detail))
        })?;
        Ok(index)
    }

    fn register(
        &mut self,
        value: &'a Value,
        inherited: &'a Value,
        path: String,
    ) -> PluginResult<()> {
        let mut pending = vec![(value, inherited, path)];
        while let Some((value, inherited, path)) = pending.pop() {
            if self.nodes.contains_key(&address(value)) {
                continue;
            }
            if self.nodes.len() == MAX_INDEX {
                return Err(budget());
            }
            let mut resource = inherited;
            if let Some(id) = value.get("$id").and_then(Value::as_str) {
                if let Some(fragment) = id.strip_prefix('#') {
                    // Fragment IDs are Draft 7 anchors, not new documents.
                    if id.len() > 1 && self.draft == Draft::Draft7 {
                        let name = decode_fragment(fragment)
                            .map_err(|detail| reference_error(&path, detail))?;
                        self.anchor(inherited, name, value, &path)?;
                    } else if id.len() > 1 {
                        return Err(reference_error(&path, "Fragment $id is unsupported"));
                    }
                } else if !id.is_empty() {
                    // Resolving arbitrary relative IDs needs full URI resource
                    // aliasing. Fail closed rather than estimate a wrong tree.
                    let id = id.strip_suffix('#').unwrap_or(id);
                    if id.contains('#') {
                        return Err(reference_error(
                            &path,
                            "$id with an embedded fragment is unsupported",
                        ));
                    }
                    let normalized = jsonschema::Uri::parse(id)
                        .map_err(|_| reference_error(&path, "Relative $id scopes are unsupported"))?
                        .normalize()
                        .to_string();
                    if self
                        .ids
                        .insert(normalized, address(value))
                        .is_some_and(|previous| previous != address(value))
                    {
                        return Err(reference_error(
                            &path,
                            "Duplicate resource $id is unsupported",
                        ));
                    }
                    resource = value;
                }
            }
            self.nodes.insert(
                address(value),
                Node {
                    value,
                    resource,
                    path: path.clone(),
                },
            );
            let Some(map) = value.as_object() else {
                continue;
            };
            for keyword in ["$anchor", "$dynamicAnchor"] {
                if let Some(name) = map.get(keyword).and_then(Value::as_str) {
                    self.anchor(resource, name.to_owned(), value, &path)?;
                    if keyword == "$dynamicAnchor" {
                        self.dynamic
                            .entry(name.to_owned())
                            .or_default()
                            .push(address(value));
                    }
                }
            }
            if map.get("$recursiveAnchor").and_then(Value::as_bool) == Some(true) {
                self.recursive.push(address(value));
            }
            for keyword in ["$ref", "$dynamicRef", "$recursiveRef"] {
                if let Some(text) = map.get(keyword).and_then(Value::as_str) {
                    if !text.starts_with('#') {
                        return Err(reference_error(
                            &guard::pointer(&path, keyword),
                            "Only document fragments are supported",
                        ));
                    }
                    self.references.push(Reference {
                        source: address(value),
                        keyword,
                        text: text.to_owned(),
                    });
                }
            }
            for keyword in [
                "properties",
                "patternProperties",
                "$defs",
                "definitions",
                "dependentSchemas",
            ] {
                if let Some(entries) = map.get(keyword).and_then(Value::as_object) {
                    let prefix = guard::pointer(&path, keyword);
                    pending.extend(
                        entries
                            .iter()
                            .map(|(name, child)| (child, resource, guard::pointer(&prefix, name))),
                    );
                }
            }
            if let Some(entries) = map.get("dependencies").and_then(Value::as_object) {
                let prefix = guard::pointer(&path, "dependencies");
                pending.extend(
                    entries
                        .iter()
                        .filter(|(_, child)| !child.is_array())
                        .map(|(name, child)| (child, resource, guard::pointer(&prefix, name))),
                );
            }
            for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
                if let Some(entries) = map.get(keyword).and_then(Value::as_array) {
                    let prefix = guard::pointer(&path, keyword);
                    pending.extend(entries.iter().enumerate().map(|(i, child)| {
                        (child, resource, guard::pointer(&prefix, &i.to_string()))
                    }));
                }
            }
            for keyword in [
                "additionalProperties",
                "additionalItems",
                "unevaluatedProperties",
                "unevaluatedItems",
                "propertyNames",
                "contains",
                "not",
                "if",
                "then",
                "else",
                "contentSchema",
                "items",
            ] {
                if let Some(child) = map.get(keyword) {
                    let prefix = guard::pointer(&path, keyword);
                    if keyword == "items" && child.is_array() {
                        pending.extend(
                            child
                                .as_array()
                                .expect("checked array")
                                .iter()
                                .enumerate()
                                .map(|(i, child)| {
                                    (child, resource, guard::pointer(&prefix, &i.to_string()))
                                }),
                        );
                    } else {
                        pending.push((child, resource, prefix));
                    }
                }
            }
        }
        Ok(())
    }

    fn anchor(
        &mut self,
        resource: &Value,
        name: String,
        value: &Value,
        path: &str,
    ) -> PluginResult<()> {
        if self
            .anchors
            .insert((address(resource), name), address(value))
            .is_some_and(|previous| previous != address(value))
        {
            return Err(reference_error(path, "Ambiguous document anchor"));
        }
        Ok(())
    }

    fn resolve(&self, source: usize, text: &str) -> PluginResult<(&'a Value, &'a Value, String)> {
        let source = &self.nodes[&source];
        let fragment =
            decode_fragment(&text[1..]).map_err(|detail| reference_error(&source.path, detail))?;
        let root = &self.nodes[&address(source.resource)];
        if fragment.is_empty() {
            return Ok((root.value, root.resource, root.path.clone()));
        }
        if !fragment.starts_with('/') {
            let id = self
                .anchors
                .get(&(address(source.resource), fragment))
                .ok_or_else(|| reference_error(&source.path, "Unknown document anchor"))?;
            let target = &self.nodes[id];
            return Ok((target.value, target.resource, target.path.clone()));
        }
        let mut value = root.value;
        let mut resource = root.resource;
        let mut path = root.path.clone();
        for encoded in fragment[1..].split('/') {
            let segment =
                decode_pointer(encoded).map_err(|detail| reference_error(&source.path, detail))?;
            value = match value {
                Value::Object(entries) => entries.get(&segment),
                Value::Array(entries) => segment.parse::<usize>().ok().and_then(|i| entries.get(i)),
                _ => None,
            }
            .ok_or_else(|| reference_error(&source.path, "Document pointer does not resolve"))?;
            path = guard::pointer(&path, &segment);
            if let Some(node) = self.nodes.get(&address(value)) {
                resource = node.resource;
            } else if value.get("$id").and_then(Value::as_str).is_some() {
                return Err(reference_error(
                    &path,
                    "Resource $id beneath an annotation or extension is unsupported",
                ));
            }
        }
        Ok((value, resource, path))
    }
}

fn decode_fragment(text: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut source = text.bytes();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let high = source.next().and_then(|b| (b as char).to_digit(16));
            let low = source.next().and_then(|b| (b as char).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("Invalid percent escape in fragment");
            };
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|_| "Fragment is not valid UTF-8")
}

fn decode_pointer(text: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            output.push(match chars.next() {
                Some('0') => '~',
                Some('1') => '/',
                _ => return Err("Invalid JSON Pointer escape"),
            });
        } else {
            output.push(character);
        }
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum Instance<'a> {
    Value(&'a Value),
    // The real property's value address identifies its virtual key string.
    Key(&'a Value),
}

impl Instance<'_> {
    fn identity(self) -> (usize, bool) {
        match self {
            Self::Value(value) => (address(value), false),
            Self::Key(value) => (address(value), true),
        }
    }
}

type Pair = (usize, usize, usize, bool);

struct Expansion<'a> {
    cost: usize,
    children: Vec<(&'a Value, Instance<'a>)>,
}

enum Visit<'a> {
    Enter {
        schema: &'a Value,
        instance: Instance<'a>,
        depth: usize,
    },
    Exit {
        pair: Pair,
        cost: usize,
        result_start: usize,
    },
}

struct Estimator<'a> {
    index: Index<'a>,
    memo: HashMap<Pair, usize>,
    active: HashSet<Pair>,
    cyclic: HashSet<Pair>,
    data_sizes: HashMap<usize, usize>,
}

impl<'a> Estimator<'a> {
    fn cost(
        &mut self,
        schema: &'a Value,
        instance: Instance<'a>,
        depth: usize,
    ) -> PluginResult<usize> {
        // Keep traversal frames on the heap: even a rejected reference chain
        // must not consume one large Rust stack frame per schema evaluation.
        let mut visits = vec![Visit::Enter {
            schema,
            instance,
            depth,
        }];
        let mut results = Vec::new();
        while let Some(visit) = visits.pop() {
            match visit {
                Visit::Enter {
                    schema,
                    instance,
                    depth,
                } => {
                    if depth > MAX_STACK {
                        return Err(budget());
                    }
                    let node = &self.index.nodes[&address(schema)];
                    let (instance_id, key) = instance.identity();
                    let pair = (address(schema), instance_id, address(node.resource), key);
                    if let Some(cost) = self.memo.get(&pair) {
                        results.push(*cost);
                        continue;
                    }
                    if !self.active.insert(pair) {
                        // Cyclic costs depend on the currently active ancestors.
                        // Do not cache a subtree cut short by a same-pair cycle.
                        for active in &self.active {
                            self.cyclic.insert(*active);
                            self.memo.remove(active);
                        }
                        results.push(1);
                        continue;
                    }
                    let expansion = self.expanded(schema, instance)?;
                    visits.push(Visit::Exit {
                        pair,
                        cost: expansion.cost,
                        result_start: results.len(),
                    });
                    // LIFO preserves the original schema traversal order so the
                    // active set and cycle-dependent accounting stay equivalent.
                    for (schema, instance) in expansion.children.into_iter().rev() {
                        visits.push(Visit::Enter {
                            schema,
                            instance,
                            depth: depth + 1,
                        });
                    }
                }
                Visit::Exit {
                    pair,
                    mut cost,
                    result_start,
                } => {
                    for child_cost in results.drain(result_start..) {
                        Self::add(&mut cost, child_cost)?;
                    }
                    self.active.remove(&pair);
                    if !self.cyclic.contains(&pair) {
                        self.memo.insert(pair, cost);
                    }
                    results.push(cost);
                }
            }
        }
        Ok(results.pop().expect("root expansion result"))
    }

    fn child(
        children: &mut Vec<(&'a Value, Instance<'a>)>,
        schema: &'a Value,
        instance: Instance<'a>,
    ) -> PluginResult<()> {
        // Prevent a wide schema from allocating an unbounded job list before
        // its aggregate cost is known. Each child contributes at least one.
        if children.len() >= MAX_WORK {
            return Err(budget());
        }
        children.push((schema, instance));
        Ok(())
    }

    fn add(total: &mut usize, cost: usize) -> PluginResult<()> {
        *total = total.checked_add(cost).ok_or_else(budget)?;
        if *total > MAX_WORK {
            return Err(budget());
        }
        Ok(())
    }

    fn data_size(&mut self, root: &Value) -> usize {
        if let Some(size) = self.data_sizes.get(&address(root)) {
            return *size;
        }
        let mut size = 0usize;
        let mut pending = vec![root];
        while let Some(value) = pending.pop() {
            size = size.saturating_add(1);
            if size > MAX_WORK {
                break;
            }
            match value {
                Value::Array(entries) => pending.extend(entries),
                Value::Object(entries) => pending.extend(entries.values()),
                _ => {}
            }
        }
        self.data_sizes.insert(address(root), size);
        size
    }

    fn expanded(
        &mut self,
        schema: &'a Value,
        instance: Instance<'a>,
    ) -> PluginResult<Expansion<'a>> {
        let Some(map) = schema.as_object() else {
            return Ok(Expansion {
                cost: 1,
                children: Vec::new(),
            });
        };
        let mut children = Vec::new();
        // Each keyword can emit its own error; count them even when they assert
        // only a scalar constraint rather than introduce another schema.
        let mut total = map.len() + 1;
        // Numeric/mixed enum validators compare candidates linearly; a short
        // schema evaluated on a large array can otherwise hide billions of
        // comparisons. Count the candidate trees, including const's one tree.
        for keyword in ["enum", "const"] {
            if let Some(candidates) = map.get(keyword) {
                let cost = self.data_size(candidates);
                Self::add(&mut total, cost)?;
            }
        }
        for keyword in ["$ref", "$dynamicRef", "$recursiveRef"] {
            if let Some(targets) = self.index.edges.get(&(address(schema), keyword)).cloned() {
                for target in targets {
                    let target = self.index.nodes[&target].value;
                    Self::child(&mut children, target, instance)?;
                }
            }
        }
        for keyword in ["allOf", "anyOf", "oneOf"] {
            if let Some(schemas) = map.get(keyword).and_then(Value::as_array) {
                for child in schemas {
                    Self::child(&mut children, child, instance)?;
                }
            }
        }
        for keyword in ["not", "if", "then", "else"] {
            if let Some(child) = map.get(keyword) {
                Self::child(&mut children, child, instance)?;
            }
        }
        let Instance::Value(value) = instance else {
            return Ok(Expansion {
                cost: total,
                children,
            });
        };
        if let Some(properties) = value.as_object() {
            if let Some(required) = map.get("required").and_then(Value::as_array) {
                Self::add(&mut total, required.len())?;
            }
            if let Some(schemas) = map.get("properties").and_then(Value::as_object) {
                for (name, child) in schemas {
                    if let Some(value) = properties.get(name) {
                        Self::child(&mut children, child, Instance::Value(value))?;
                    }
                }
            }
            if let Some(patterns) = map.get("patternProperties").and_then(Value::as_object) {
                // Any pattern may match any key; do not run a regex in preflight.
                for child in patterns.values() {
                    for value in properties.values() {
                        Self::child(&mut children, child, Instance::Value(value))?;
                    }
                }
            }
            for keyword in [
                "additionalProperties",
                "unevaluatedProperties",
                "propertyNames",
            ] {
                if let Some(child) = map.get(keyword) {
                    for value in properties.values() {
                        let instance = if keyword == "propertyNames" {
                            Instance::Key(value)
                        } else {
                            Instance::Value(value)
                        };
                        Self::child(&mut children, child, instance)?;
                    }
                }
            }
            for keyword in ["dependentSchemas", "dependencies"] {
                if let Some(schemas) = map.get(keyword).and_then(Value::as_object) {
                    for (name, child) in schemas {
                        if properties.contains_key(name) && !child.is_array() {
                            Self::child(&mut children, child, instance)?;
                        } else if properties.contains_key(name)
                            && let Some(required) = child.as_array()
                        {
                            Self::add(&mut total, required.len())?;
                        }
                    }
                }
            }
            if let Some(dependencies) = map.get("dependentRequired").and_then(Value::as_object) {
                for (name, required) in dependencies {
                    if properties.contains_key(name)
                        && let Some(required) = required.as_array()
                    {
                        Self::add(&mut total, required.len())?;
                    }
                }
            }
        }
        if let Some(items) = value.as_array() {
            if map.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
                // 0.58.6 compares pairs for <=15 entries and hashes larger
                // arrays (jsonschema-value::unique::is_unique).
                let cost = self.data_size(value).saturating_mul(if items.len() <= 15 {
                    items.len()
                } else {
                    1
                });
                Self::add(&mut total, cost)?;
            }
            for keyword in ["prefixItems", "items"] {
                if let Some(child) = map.get(keyword) {
                    if let Some(tuple) = child.as_array() {
                        for (child, value) in tuple.iter().zip(items) {
                            Self::child(&mut children, child, Instance::Value(value))?;
                        }
                    } else {
                        for value in items {
                            Self::child(&mut children, child, Instance::Value(value))?;
                        }
                    }
                }
            }
            for keyword in ["contains", "additionalItems", "unevaluatedItems"] {
                if let Some(child) = map.get(keyword) {
                    for value in items {
                        Self::child(&mut children, child, Instance::Value(value))?;
                    }
                }
            }
        }
        Ok(Expansion {
            cost: total,
            children,
        })
    }
}

fn max_pointer_bytes(root: &Value) -> usize {
    let mut pending = vec![(root, 0usize)];
    let mut longest = 0;
    while let Some((value, length)) = pending.pop() {
        longest = longest.max(length);
        match value {
            Value::Object(entries) => pending.extend(entries.iter().map(|(name, value)| {
                let escaped = name.len()
                    + name
                        .bytes()
                        .filter(|byte| matches!(byte, b'~' | b'/'))
                        .count();
                (value, length.saturating_add(escaped + 1))
            })),
            Value::Array(entries) => pending.extend(
                entries
                    .iter()
                    .enumerate()
                    .map(|(i, value)| (value, length.saturating_add(i.to_string().len() + 1))),
            ),
            _ => {}
        }
    }
    longest
}

/// Reject excessive repeated evaluation before `jsonschema::iter_errors`, which
/// eagerly constructs every error even when its returned iterator is truncated.
/// Relative `$id` resource scopes and ambiguous anchors fail closed.
pub fn check(schema: &Value, instance: &Value, draft: Draft) -> PluginResult<usize> {
    let index = Index::build(schema, draft)?;
    let mut estimator = Estimator {
        index,
        memo: HashMap::new(),
        active: HashSet::new(),
        cyclic: HashSet::new(),
        data_sizes: HashMap::new(),
    };
    let cost = estimator.cost(schema, Instance::Value(instance), 0)?;
    let bytes_per_error = max_pointer_bytes(schema)
        .saturating_add(max_pointer_bytes(instance))
        .saturating_add(128);
    if cost.saturating_mul(bytes_per_error) > MAX_ERROR_BYTES {
        return Err(budget());
    }
    Ok(cost)
}

#[cfg(test)]
#[path = "work_test.rs"]
mod tests;
