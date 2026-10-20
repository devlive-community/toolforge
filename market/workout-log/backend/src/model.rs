use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

pub const MAX_DOCUMENT: usize = 700_000;
pub const INDEX_SIZE: u64 = 128;
pub const MAX_SAFE: u64 = 9_007_199_254_740_990;

pub fn error(field: &str) -> PluginError {
    PluginError::new("workout.invalid").with("field", field)
}
pub fn text(value: &str, field: &str, max: usize, required: bool) -> PluginResult<()> {
    if (required && value.trim().is_empty()) || value.chars().count() > max || value.contains('\0')
    {
        return Err(error(field));
    }
    Ok(())
}
pub fn date(value: &str) -> PluginResult<()> {
    let parsed = value
        .parse::<jiff::civil::Date>()
        .map_err(|_| error("date"))?;
    if parsed.to_string() != value || !(1900..=9999).contains(&parsed.year()) {
        return Err(error("date"));
    }
    Ok(())
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Head {
    pub version: u8,
    pub revision: u64,
    #[serde(default)]
    pub restore_source: Option<String>,
    pub next_id: u64,
    pub next_workout: u64,
    pub exercises: Vec<Exercise>,
    pub templates: Vec<Template>,
}
impl Head {
    pub fn empty() -> Self {
        Self {
            version: 1,
            next_id: 1,
            next_workout: 1,
            ..Self::default()
        }
    }
    pub fn id(&mut self, prefix: &str) -> PluginResult<String> {
        if self.next_id >= MAX_SAFE {
            return Err(error("nextId"));
        }
        let id = format!("{prefix}-{}", self.next_id);
        self.next_id += 1;
        Ok(id)
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    External,
    Bodyweight,
    Unknown,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    #[default]
    Kg,
    Lb,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Pending,
    Done,
    Skipped,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Exercise {
    pub id: String,
    pub name: String,
    pub equipment: String,
    pub mode: Mode,
    pub archived: bool,
    pub history: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub exercise: Exercise,
    pub sets: Vec<Set>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Set {
    pub reps: String,
    pub weight: String,
    pub unit: Unit,
    pub warmup: bool,
    pub note: String,
    pub status: Status,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub entries: Vec<Entry>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Body {
    pub name: String,
    pub date: String,
    pub note: String,
    pub finished: bool,
    pub entries: Vec<Entry>,
    pub rest: Option<Rest>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rest {
    pub started: i64,
    pub seconds: u32,
    pub stopped: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workout {
    pub id: u64,
    pub body: Body,
    pub undo: Option<Body>,
}
impl Workout {
    pub fn remember(&mut self) {
        self.undo = Some(self.body.clone());
    }
}
pub fn weight(value: &str) -> PluginResult<Option<f64>> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let value = value.trim();
    if value.len() > 12
        || value.matches('.').count() > 1
        || !value.bytes().all(|v| v.is_ascii_digit() || v == b'.')
        || value.split('.').nth(1).is_some_and(|s| s.len() > 3)
    {
        return Err(error("weight"));
    }
    let number = value.parse::<f64>().map_err(|_| error("weight"))?;
    if !number.is_finite() || !(0.0..=10_000.0).contains(&number) {
        return Err(error("weight"));
    }
    Ok(Some(number))
}
pub fn repetitions(value: &str) -> PluginResult<Option<u32>> {
    if value.is_empty() {
        return Ok(None);
    }
    if !value.bytes().all(|v| v.is_ascii_digit()) {
        return Err(error("reps"));
    }
    let number = value.parse::<u32>().map_err(|_| error("reps"))?;
    if number > 10_000 {
        return Err(error("reps"));
    }
    Ok(Some(number))
}
pub fn validate_set(set: &Set, mode: Mode) -> PluginResult<()> {
    let reps = repetitions(&set.reps)?;
    let load = weight(&set.weight)?;
    text(&set.note, "note", 1000, false)?;
    if set.status == Status::Done && (reps.is_none() || (mode == Mode::External && load.is_none()))
    {
        return Err(PluginError::new("workout.incompleteSet"));
    }
    if mode != Mode::External && load.is_some() {
        return Err(error("weight"));
    }
    Ok(())
}
fn exercise(e: &Exercise) -> PluginResult<()> {
    if !e.id.starts_with("exercise-") || e.history > MAX_SAFE {
        return Err(error("exercise"));
    }
    text(&e.id, "id", 50, true)?;
    text(&e.name, "name", 100, true)?;
    text(&e.equipment, "equipment", 120, false)
}
fn entries(values: &[Entry]) -> PluginResult<()> {
    if values.len() > 64 {
        return Err(PluginError::new("workout.limit").with("field", "exercises"));
    }
    let mut seen = HashSet::new();
    for entry in values {
        exercise(&entry.exercise)?;
        if !seen.insert(&entry.exercise.id) {
            return Err(error("exercise"));
        }
        if entry.sets.len() > 100 {
            return Err(PluginError::new("workout.limit").with("field", "sets"));
        }
        for set in &entry.sets {
            validate_set(set, entry.exercise.mode)?;
        }
    }
    Ok(())
}
pub fn validate_head(head: &Head) -> PluginResult<()> {
    if head.version != 1 {
        return Err(PluginError::new("workout.version"));
    }
    if head.next_id == 0
        || head.next_id > MAX_SAFE
        || head.next_workout == 0
        || head.next_workout > MAX_SAFE
        || head.revision > MAX_SAFE
    {
        return Err(error("sequence"));
    }
    let mut seen = HashSet::new();
    for e in &head.exercises {
        exercise(e)?;
        if !seen.insert(&e.id) {
            return Err(error("id"));
        }
    }
    for t in &head.templates {
        text(&t.id, "id", 50, true)?;
        text(&t.name, "name", 100, true)?;
        entries(&t.entries)?;
        if !seen.insert(&t.id) {
            return Err(error("id"));
        }
    }
    budget(head)
}
fn body(value: &Body) -> PluginResult<()> {
    text(&value.name, "name", 100, true)?;
    text(&value.note, "note", 4000, false)?;
    date(&value.date)?;
    entries(&value.entries)?;
    if let Some(rest) = &value.rest {
        validate_rest(rest)?;
    }
    Ok(())
}
pub fn validate_rest(rest: &Rest) -> PluginResult<()> {
    if rest.seconds == 0
        || rest.seconds > 3600
        || rest.started < 0
        || rest.started >= MAX_SAFE as i64
    {
        return Err(error("rest"));
    }
    Ok(())
}
pub fn validate_workout(value: &Workout) -> PluginResult<()> {
    if value.id == 0 || value.id >= MAX_SAFE {
        return Err(error("id"));
    }
    body(&value.body)?;
    if let Some(previous) = &value.undo {
        body(previous)?;
    }
    budget(value)
}
pub fn budget<T: Serialize>(value: &T) -> PluginResult<()> {
    if serde_json::to_vec(value)
        .map_err(|_| error("document"))?
        .len()
        > MAX_DOCUMENT
    {
        return Err(PluginError::new("workout.documentLimit"));
    }
    Ok(())
}
pub fn kilograms(set: &Set) -> PluginResult<Option<f64>> {
    Ok(weight(&set.weight)?.map(|v| {
        if set.unit == Unit::Lb {
            v * 0.453_592_37
        } else {
            v
        }
    }))
}
#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
