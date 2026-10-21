use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

pub const MAX_ROWS: u32 = 100_000;
pub const MAX_STITCHES: u32 = 1_000_000;
pub const MAX_STATE: usize = 8 * 1024 * 1024;
pub const MAX_EXPORT: usize = 32 * 1024 * 1024;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("knitting.{code}"))
}
pub fn check_cancel(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(tf_plugin_api::cancelled())
    } else {
        Ok(())
    }
}

pub fn id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{nanos}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

pub fn label(value: &str) -> PluginResult<String> {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > 80 || text.chars().any(char::is_control) {
        return Err(error("name"));
    }
    Ok(text.to_owned())
}

pub fn integer(text: &str, zero: bool, limit: u32) -> PluginResult<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error("number"));
    }
    let n = text.parse::<u32>().map_err(|_| error("number"))?;
    if n > limit || (!zero && n == 0) {
        return Err(error("number").with("limit", limit));
    }
    Ok(n)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Rows,
    Rounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Choice {
    Execute,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Segment {
    pub id: String,
    pub name: String,
    pub start: u32,
    pub rows: u32,
    pub repeats: u32,
}
impl Segment {
    pub fn end(&self) -> u32 {
        self.start + self.rows * self.repeats - 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reminder {
    pub id: String,
    pub name: String,
    pub start: u32,
    pub interval: u32,
    pub repeats: u32,
    pub delta: i32,
}
impl Reminder {
    pub fn occurrence(&self, line: u32) -> Option<u32> {
        if line < self.start {
            return None;
        }
        let difference = line - self.start;
        (difference.is_multiple_of(self.interval) && difference / self.interval < self.repeats)
            .then_some(difference / self.interval + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub completed: u32,
    pub stitches: u32,
    pub initial_stitches: u32,
    pub pending: BTreeMap<String, Choice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub note: String,
    pub initial_stitches: u32,
    pub completed: u32,
    pub stitches: u32,
    pub segments: Vec<Segment>,
    pub reminders: Vec<Reminder>,
    pub pending: BTreeMap<String, Choice>,
    pub undo: Vec<Snapshot>,
}

impl Part {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            completed: self.completed,
            stitches: self.stitches,
            initial_stitches: self.initial_stitches,
            pending: self.pending.clone(),
        }
    }
    pub fn remember(&mut self) -> PluginResult<()> {
        if self.undo.len() >= MAX_ROWS as usize {
            return Err(error("history_limit"));
        }
        self.undo.push(self.snapshot());
        Ok(())
    }
    pub fn due(&self) -> impl Iterator<Item = &Reminder> {
        self.reminders
            .iter()
            .filter(|r| r.occurrence(self.completed + 1).is_some())
    }
    pub fn confirmed_stitches(&self) -> PluginResult<u32> {
        let mut stitches = i64::from(self.stitches);
        for rule in self.due() {
            match self.pending.get(&rule.id) {
                Some(Choice::Execute) => {
                    stitches += i64::from(rule.delta);
                    if !(0..=i64::from(MAX_STITCHES)).contains(&stitches) {
                        return Err(error("stitches"));
                    }
                }
                Some(Choice::Skip) => {}
                None => return Err(error("pending")),
            }
        }
        Ok(stitches as u32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub active_part_id: Option<String>,
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workshop {
    pub schema_version: u32,
    pub revision: u32,
    pub active_project_id: Option<String>,
    pub projects: Vec<Project>,
}
impl Default for Workshop {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            active_project_id: None,
            projects: vec![],
        }
    }
}

impl Workshop {
    pub fn active(&self) -> Option<(&Project, &Part)> {
        let project = self
            .projects
            .iter()
            .find(|p| Some(&p.id) == self.active_project_id.as_ref())?;
        let part = project
            .parts
            .iter()
            .find(|p| Some(&p.id) == project.active_part_id.as_ref())?;
        Some((project, part))
    }
    pub fn project_mut(&mut self) -> PluginResult<&mut Project> {
        self.projects
            .iter_mut()
            .find(|p| Some(&p.id) == self.active_project_id.as_ref())
            .ok_or_else(|| error("selection"))
    }
    pub fn part_mut(&mut self) -> PluginResult<&mut Part> {
        let project = self.project_mut()?;
        project
            .parts
            .iter_mut()
            .find(|p| Some(&p.id) == project.active_part_id.as_ref())
            .ok_or_else(|| error("selection"))
    }
}

fn valid_id(id: &str, ids: &mut BTreeSet<String>) -> PluginResult<()> {
    if id.is_empty() || id.len() > 100 || !ids.insert(id.to_owned()) {
        Err(error("invalid_state"))
    } else {
        Ok(())
    }
}
fn valid_pending(part: &Part, completed: u32, pending: &BTreeMap<String, Choice>) -> bool {
    pending.keys().all(|key| {
        part.reminders
            .iter()
            .any(|r| &r.id == key && r.occurrence(completed + 1).is_some())
    })
}

pub fn validate(state: &Workshop, ctx: &dyn TaskContext) -> PluginResult<()> {
    if state.schema_version != 1 {
        return Err(error("version"));
    }
    if state.projects.len() > 50 || state.revision > 1_000_000_000 {
        return Err(error("limits"));
    }
    let mut ids = BTreeSet::new();
    for project in &state.projects {
        check_cancel(ctx)?;
        valid_id(&project.id, &mut ids)?;
        label(&project.name)?;
        if project.parts.len() > 20 {
            return Err(error("limits"));
        }
        if project.parts.is_empty() != project.active_part_id.is_none()
            || project
                .active_part_id
                .as_ref()
                .is_some_and(|id| !project.parts.iter().any(|p| &p.id == id))
        {
            return Err(error("invalid_state"));
        }
        for part in &project.parts {
            valid_id(&part.id, &mut ids)?;
            label(&part.name)?;
            if part.note.chars().count() > 2000
                || part.initial_stitches > MAX_STITCHES
                || part.stitches > MAX_STITCHES
                || part.completed > MAX_ROWS
                || part.segments.len() > 20
                || part.reminders.len() > 20
                || part.undo.len() > MAX_ROWS as usize
            {
                return Err(error("limits"));
            }
            let mut last_end = 0;
            for segment in &part.segments {
                valid_id(&segment.id, &mut ids)?;
                if segment.name.chars().count() > 80
                    || segment.start == 0
                    || segment.rows == 0
                    || segment.repeats == 0
                    || segment.repeats > 10_000
                    || segment
                        .rows
                        .checked_mul(segment.repeats)
                        .and_then(|n| segment.start.checked_add(n - 1))
                        .is_none_or(|end| end > MAX_ROWS)
                    || segment.start <= last_end
                {
                    return Err(error("segment"));
                }
                last_end = segment.end();
            }
            for reminder in &part.reminders {
                valid_id(&reminder.id, &mut ids)?;
                if reminder.name.chars().count() > 80
                    || reminder.start == 0
                    || reminder.interval == 0
                    || reminder.interval > MAX_ROWS
                    || reminder.repeats == 0
                    || reminder.repeats > 10_000
                    || reminder.delta == 0
                    || reminder.delta.unsigned_abs() > MAX_STITCHES
                    || reminder
                        .interval
                        .checked_mul(reminder.repeats - 1)
                        .and_then(|n| reminder.start.checked_add(n))
                        .is_none_or(|end| end > MAX_ROWS)
                {
                    return Err(error("reminder"));
                }
            }
            if !valid_pending(part, part.completed, &part.pending) {
                return Err(error("invalid_state"));
            }
            for (index, snapshot) in part.undo.iter().enumerate() {
                if index.is_multiple_of(256) {
                    check_cancel(ctx)?;
                }
                if snapshot.completed > MAX_ROWS
                    || snapshot.stitches > MAX_STITCHES
                    || snapshot.initial_stitches > MAX_STITCHES
                    || !valid_pending(part, snapshot.completed, &snapshot.pending)
                {
                    return Err(error("invalid_state"));
                }
                let after = part
                    .undo
                    .get(index + 1)
                    .map_or(part.completed, |s| s.completed);
                if after < snapshot.completed || after > snapshot.completed + 1 {
                    return Err(error("invalid_state"));
                }
            }
        }
    }
    if state.projects.is_empty() != state.active_project_id.is_none()
        || state
            .active_project_id
            .as_ref()
            .is_some_and(|id| !state.projects.iter().any(|p| &p.id == id))
    {
        return Err(error("invalid_state"));
    }
    if serde_json::to_vec(state)
        .map_err(|_| error("invalid_state"))?
        .len()
        > MAX_STATE
    {
        return Err(error("state_limit"));
    }
    Ok(())
}
