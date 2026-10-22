use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};
use tf_plugin_api::PluginResult;

pub fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}
#[derive(Clone)]
pub struct Headers {
    pub fields: Vec<Header>,
    pub malformed: bool,
}
impl Headers {
    pub fn parse(text: &str, context: &str, notes: &mut Vec<Note>) -> PluginResult<Self> {
        if text.len() > HEADER_BYTES {
            return Err(error("headers_size"));
        }
        let mut fields = Vec::new();
        let mut malformed = false;
        let mut count = 0;
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            count += 1;
            if count > FIELD_LIMIT {
                return Err(error("fields_limit"));
            }
            let Some((name, value)) = line.split_once(':').filter(|(name, _)| token(name)) else {
                malformed = true;
                notes.push(
                    Note::new("header_syntax")
                        .with("context", context)
                        .with("line", line_no + 1),
                );
                continue;
            };
            let value = value.trim_matches([' ', '\t']);
            if value.bytes().any(|b| b < 32 && b != b'\t' || b == 127) {
                malformed = true;
                notes.push(
                    Note::new("header_syntax")
                        .with("context", context)
                        .with("line", line_no + 1),
                );
                continue;
            }
            fields.push(Header {
                name: name.to_ascii_lowercase(),
                value: value.to_owned(),
            });
        }
        Ok(Self { fields, malformed })
    }
    pub fn values(&self, name: &str) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|h| h.name == name)
            .map(|h| h.value.as_str())
            .collect()
    }
    pub fn has(&self, name: &str) -> bool {
        self.fields.iter().any(|h| h.name == name)
    }
    pub fn one<'a>(&'a self, name: &str, notes: &mut Vec<Note>) -> Option<&'a str> {
        let values = self.values(name);
        if values.len() > 1 {
            notes.push(Note::new("singleton_duplicate").with("field", name));
            None
        } else {
            values.first().copied()
        }
    }
    pub fn redacted(&self) -> Vec<Header> {
        self.fields
            .iter()
            .map(|h| Header {
                name: h.name.clone(),
                value: masked(&h.name, &h.value),
            })
            .collect()
    }
}

// Split HTTP lists without treating commas inside quoted strings as separators.
pub fn list(text: &str) -> Option<Vec<&str>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quoted {
            escaped = true;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if c == ',' && !quoted {
            result.push(text[start..i].trim());
            start = i + 1;
        }
    }
    if quoted || escaped {
        return None;
    }
    result.push(text[start..].trim());
    Some(result)
}
fn argument(text: &str) -> Option<String> {
    if token(text) {
        return Some(text.to_owned());
    }
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut escaped = false;
    for c in inner.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' || c.is_control() && c != '\t' {
            return None;
        } else {
            out.push(c);
        }
    }
    (!escaped).then_some(out)
}
pub fn delta(text: &str, notes: &mut Vec<Note>, field: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut n = 0u64;
    for b in text.bytes() {
        n = n.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    if n > DELTA_LIMIT {
        notes.push(
            Note::new("delta_clamped")
                .with("field", field)
                .with("limit", DELTA_LIMIT),
        );
        n = DELTA_LIMIT;
    }
    Some(n)
}

pub struct Control {
    pub entries: Vec<Directive>,
    pub invalid: BTreeSet<String>,
    pub malformed: bool,
    pub boundary: bool,
}
impl Control {
    pub fn parse(headers: &Headers, context: &str, response: bool, notes: &mut Vec<Note>) -> Self {
        let mut entries = Vec::new();
        let mut invalid = BTreeSet::new();
        let mut malformed = false;
        let mut boundary = false;
        for raw in headers.values("cache-control") {
            let Some(parts) = list(raw) else {
                malformed = true;
                notes.push(Note::new("control_syntax").with("context", context));
                continue;
            };
            for part in parts {
                if part.is_empty() {
                    continue;
                }
                let (name, arg) = part
                    .split_once('=')
                    .map_or((part, None), |(n, v)| (n, Some(v.trim())));
                let name = name.trim().to_ascii_lowercase();
                let value = arg.and_then(argument);
                let known = if response {
                    [
                        "max-age",
                        "s-maxage",
                        "no-store",
                        "no-cache",
                        "private",
                        "public",
                        "must-revalidate",
                        "proxy-revalidate",
                        "no-transform",
                        "must-understand",
                    ]
                    .contains(&name.as_str())
                } else {
                    [
                        "max-age",
                        "min-fresh",
                        "no-cache",
                        "no-store",
                        "no-transform",
                        "only-if-cached",
                    ]
                    .contains(&name.as_str())
                };
                let note_start = notes.len();
                let valid = token(&name)
                    && (arg.is_none() || value.is_some())
                    && if ["max-age", "s-maxage", "min-fresh"].contains(&name.as_str()) {
                        value
                            .as_ref()
                            .is_some_and(|v| delta(v, notes, &name).is_some())
                    } else if response && ["private", "no-cache"].contains(&name.as_str()) {
                        true
                    } else if known {
                        value.is_none()
                    } else {
                        true
                    };
                for note in &mut notes[note_start..] {
                    note.params["context"] = context.into();
                }
                let supported = known
                    && !(response
                        && ["private", "no-cache"].contains(&name.as_str())
                        && value.is_some());
                if !valid {
                    invalid.insert(name.clone());
                    notes.push(
                        Note::new("directive_invalid")
                            .with("context", context)
                            .with("field", name.as_str()),
                    );
                }
                if !supported {
                    boundary = true;
                    notes.push(
                        Note::new("directive_boundary")
                            .with("context", context)
                            .with("field", name.as_str()),
                    );
                }
                entries.push(Directive {
                    context: context.into(),
                    name,
                    value,
                    valid,
                    supported,
                });
            }
        }
        let mut counts = BTreeMap::new();
        for entry in &entries {
            *counts.entry(entry.name.clone()).or_insert(0usize) += 1;
        }
        for (name, count) in counts {
            if count > 1 {
                invalid.insert(name.clone());
                notes.push(
                    Note::new("directive_duplicate")
                        .with("context", context)
                        .with("field", name),
                );
            }
        }
        if entries.len() > 256 {
            malformed = true;
            notes.push(Note::new("directives_limit"));
        }
        Self {
            entries,
            invalid,
            malformed,
            boundary,
        }
    }
    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }
    pub fn flag(&self, name: &str) -> bool {
        !self.invalid.contains(name)
            && self
                .entries
                .iter()
                .any(|e| e.name == name && e.valid && e.value.is_none())
    }
    pub fn qualified(&self, name: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.name == name && e.value.is_some())
    }
    pub fn number(&self, name: &str) -> Option<u64> {
        if self.invalid.contains(name) {
            return None;
        }
        self.entries
            .iter()
            .find(|e| e.name == name && e.valid)
            .and_then(|e| e.value.as_deref())
            .and_then(|v| delta(v, &mut Vec::new(), name))
    }
}

pub fn vary(
    response: &Headers,
    original: &Headers,
    presented: &Headers,
    notes: &mut Vec<Note>,
) -> (String, Vec<VaryRow>) {
    let mut fields = BTreeSet::new();
    let mut bad = false;
    for value in response.values("vary") {
        for name in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
            if name != "*" && !token(name) {
                bad = true;
            } else {
                fields.insert(name.to_ascii_lowercase());
            }
        }
    }
    if fields.len() > FIELD_LIMIT {
        bad = true;
    }
    if bad {
        notes.push(Note::new("vary_invalid"));
        return ("unknown".into(), Vec::new());
    }
    let mut rows = Vec::new();
    let mut different = false;
    for name in fields {
        let a = original.values(&name);
        let b = presented.values(&name);
        // Exact field-line equality is safe for every field. Combining common
        // list fields is also safe; arbitrary field values are never lowercased.
        let equal = a == b
            || ["accept", "accept-encoding", "accept-language"].contains(&name.as_str())
                && a.join(", ") == b.join(", ");
        let equal = equal && name != "*";
        different |= !equal;
        rows.push(VaryRow {
            name: name.clone(),
            stored: (!a.is_empty()).then(|| masked(&name, &a.join(", "))),
            presented: (!b.is_empty()).then(|| masked(&name, &b.join(", "))),
            status: if equal { "match" } else { "different" }.into(),
        });
    }
    if different {
        notes.push(Note::new("vary_conservative"));
    }
    (if different { "different" } else { "match" }.into(), rows)
}
