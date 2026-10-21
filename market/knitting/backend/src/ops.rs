use crate::model::*;
use serde::Deserialize;
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Op {
    NewProject {
        name: String,
        part_name: String,
        kind: Kind,
        stitches: String,
    },
    NewPart {
        name: String,
        kind: Kind,
        stitches: String,
    },
    SelectProject {
        id: String,
    },
    SelectPart {
        id: String,
    },
    Details {
        project_name: String,
        part_name: String,
        note: String,
    },
    AddSegment {
        name: String,
        rows: String,
        repeats: String,
    },
    AddReminder {
        name: String,
        start: String,
        interval: String,
        repeats: String,
        delta: String,
    },
    RemoveSegment {
        id: String,
    },
    RemoveReminder {
        id: String,
    },
    Choose {
        id: String,
        choice: Option<Choice>,
    },
    Complete {
        expected_line: u32,
    },
    Undo,
    Adjust {
        stitches: String,
    },
    CopyPart {
        name: String,
    },
    Clear {
        confirm: bool,
    },
    DeletePart {
        confirm: bool,
    },
    DeleteProject {
        confirm: bool,
    },
}

fn new_part(name: &str, kind: Kind, stitches: &str) -> PluginResult<Part> {
    let stitches = integer(stitches, true, MAX_STITCHES)?;
    Ok(Part {
        id: id(),
        name: label(name)?,
        kind,
        note: String::new(),
        initial_stitches: stitches,
        completed: 0,
        stitches,
        segments: vec![],
        reminders: vec![],
        pending: Default::default(),
        undo: vec![],
    })
}

pub fn apply(mut state: Workshop, op: Op, ctx: &dyn TaskContext) -> PluginResult<Workshop> {
    validate(&state, ctx)?;
    check_cancel(ctx)?;
    match op {
        Op::NewProject {
            name,
            part_name,
            kind,
            stitches,
        } => {
            if state.projects.len() >= 50 {
                return Err(error("limits"));
            }
            let part = new_part(&part_name, kind, &stitches)?;
            let project = Project {
                id: id(),
                name: label(&name)?,
                active_part_id: Some(part.id.clone()),
                parts: vec![part],
            };
            state.active_project_id = Some(project.id.clone());
            state.projects.push(project);
        }
        Op::NewPart {
            name,
            kind,
            stitches,
        } => {
            let part = new_part(&name, kind, &stitches)?;
            let project = state.project_mut()?;
            if project.parts.len() >= 20 {
                return Err(error("limits"));
            }
            project.active_part_id = Some(part.id.clone());
            project.parts.push(part);
        }
        Op::SelectProject { id } => {
            if !state.projects.iter().any(|p| p.id == id) {
                return Err(error("selection"));
            }
            state.active_project_id = Some(id);
        }
        Op::SelectPart { id } => {
            let project = state.project_mut()?;
            if !project.parts.iter().any(|p| p.id == id) {
                return Err(error("selection"));
            }
            project.active_part_id = Some(id);
        }
        Op::Details {
            project_name,
            part_name,
            note,
        } => {
            if note.chars().count() > 2000 {
                return Err(error("limits"));
            }
            state.project_mut()?.name = label(&project_name)?;
            let part = state.part_mut()?;
            part.name = label(&part_name)?;
            part.note = note;
        }
        Op::AddSegment {
            name,
            rows,
            repeats,
        } => {
            let rows = integer(&rows, false, MAX_ROWS)?;
            let repeats = integer(&repeats, false, 10_000)?;
            let part = state.part_mut()?;
            if part.segments.len() >= 20 || name.chars().count() > 80 {
                return Err(error("limits"));
            }
            let start = (part.completed + 1).max(part.segments.last().map_or(1, |s| s.end() + 1));
            if rows
                .checked_mul(repeats)
                .and_then(|n| start.checked_add(n - 1))
                .is_none_or(|end| end > MAX_ROWS)
            {
                return Err(error("segment"));
            }
            part.segments.push(Segment {
                id: id(),
                name: name.trim().into(),
                start,
                rows,
                repeats,
            });
        }
        Op::AddReminder {
            name,
            start,
            interval,
            repeats,
            delta,
        } => {
            let start = integer(&start, false, MAX_ROWS)?;
            let interval = integer(&interval, false, MAX_ROWS)?;
            let repeats = integer(&repeats, false, 10_000)?;
            let delta = delta.trim().parse::<i32>().map_err(|_| error("reminder"))?;
            let part = state.part_mut()?;
            if start <= part.completed {
                return Err(error("past_rule"));
            }
            if name.chars().count() > 80 || part.reminders.len() >= 20 {
                return Err(error("limits"));
            }
            part.reminders.push(Reminder {
                id: id(),
                name: name.trim().into(),
                start,
                interval,
                repeats,
                delta,
            });
        }
        Op::RemoveSegment { id } => {
            let part = state.part_mut()?;
            if part.completed != 0 || !part.undo.is_empty() {
                return Err(error("edit_locked"));
            }
            if !part.segments.iter().any(|s| s.id == id) {
                return Err(error("selection"));
            }
            part.segments.retain(|s| s.id != id);
        }
        Op::RemoveReminder { id } => {
            let part = state.part_mut()?;
            if part.completed != 0 || !part.undo.is_empty() {
                return Err(error("edit_locked"));
            }
            if !part.reminders.iter().any(|r| r.id == id) {
                return Err(error("selection"));
            }
            part.reminders.retain(|r| r.id != id);
            part.pending.remove(&id);
        }
        Op::Choose { id, choice } => {
            let part = state.part_mut()?;
            if !part.due().any(|r| r.id == id) {
                return Err(error("selection"));
            }
            if let Some(choice) = choice {
                part.pending.insert(id, choice);
            } else {
                part.pending.remove(&id);
            }
        }
        Op::Complete { expected_line } => {
            let part = state.part_mut()?;
            if expected_line != part.completed + 1 {
                return Err(error("stale_line"));
            }
            if part.completed == MAX_ROWS {
                return Err(error("row_limit"));
            }
            let stitches = part.confirmed_stitches()?;
            part.remember()?;
            part.completed += 1;
            part.stitches = stitches;
            part.pending.clear();
        }
        Op::Undo => {
            let part = state.part_mut()?;
            let previous = part.undo.pop().ok_or_else(|| error("no_undo"))?;
            part.completed = previous.completed;
            part.stitches = previous.stitches;
            part.initial_stitches = previous.initial_stitches;
            part.pending = previous.pending;
        }
        Op::Adjust { stitches } => {
            let stitches = integer(&stitches, true, MAX_STITCHES)?;
            let part = state.part_mut()?;
            part.remember()?;
            part.stitches = stitches;
            if part.completed == 0 {
                part.initial_stitches = stitches;
            }
        }
        Op::CopyPart { name } => {
            let source = state.active().ok_or_else(|| error("selection"))?.1;
            let mut part = new_part(&name, source.kind, &source.initial_stitches.to_string())?;
            part.note = source.note.clone();
            part.segments = source
                .segments
                .iter()
                .cloned()
                .map(|mut s| {
                    s.id = id();
                    s
                })
                .collect();
            part.reminders = source
                .reminders
                .iter()
                .cloned()
                .map(|mut r| {
                    r.id = id();
                    r
                })
                .collect();
            let project = state.project_mut()?;
            if project.parts.len() >= 20 {
                return Err(error("limits"));
            }
            project.active_part_id = Some(part.id.clone());
            project.parts.push(part);
        }
        Op::Clear { confirm } => {
            if !confirm {
                return Err(error("confirmation"));
            }
            let part = state.part_mut()?;
            part.completed = 0;
            part.stitches = part.initial_stitches;
            part.pending.clear();
            part.undo.clear();
        }
        Op::DeletePart { confirm } => {
            if !confirm {
                return Err(error("confirmation"));
            }
            let project = state.project_mut()?;
            let selected = project
                .active_part_id
                .clone()
                .ok_or_else(|| error("selection"))?;
            project.parts.retain(|p| p.id != selected);
            project.active_part_id = project.parts.first().map(|p| p.id.clone());
        }
        Op::DeleteProject { confirm } => {
            if !confirm {
                return Err(error("confirmation"));
            }
            let selected = state
                .active_project_id
                .clone()
                .ok_or_else(|| error("selection"))?;
            state.projects.retain(|p| p.id != selected);
            state.active_project_id = state.projects.first().map(|p| p.id.clone());
        }
    }
    state.revision = state
        .revision
        .checked_add(1)
        .ok_or_else(|| error("limits"))?;
    validate(&state, ctx)?;
    check_cancel(ctx)?;
    Ok(state)
}

#[cfg(test)]
#[path = "ops_test.rs"]
mod tests;
