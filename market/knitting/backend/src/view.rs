use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tf_plugin_api::{PluginResult, TaskContext};

pub const PAGE_SIZE: usize = 5;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Pages {
    pub projects: usize,
    pub parts: usize,
    pub segments: usize,
    pub reminders: usize,
    pub due: usize,
    pub history: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}
fn page<T>(items: Vec<T>, offset: usize) -> Page<T> {
    let total = items.len();
    let offset = offset.min(total.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE);
    Page {
        items: items.into_iter().skip(offset).take(PAGE_SIZE).collect(),
        total,
        offset,
        limit: PAGE_SIZE,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectItem {
    pub id: String,
    pub name: String,
    pub part_count: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartItem {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub completed: u32,
    pub stitches: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub start: u32,
    pub end: u32,
    pub row: u32,
    pub repeat: u32,
    pub repeats: u32,
    pub rows: u32,
    pub completed_repeats: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentItem {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub start: u32,
    pub end: u32,
    pub rows: u32,
    pub repeats: u32,
    pub completed_repeats: u32,
    pub completed_rows: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderItem {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub start: u32,
    pub interval: u32,
    pub repeats: u32,
    pub delta: i32,
    pub next: Option<u32>,
    pub executed: u32,
    pub skipped: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueItem {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub delta: i32,
    pub occurrence: u32,
    pub repeats: u32,
    pub choice: Option<Choice>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub index: usize,
    pub kind: &'static str,
    pub completed_before: u32,
    pub completed_after: u32,
    pub stitches_before: u32,
    pub stitches_after: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub note: String,
    pub completed: u32,
    pub next_line: u32,
    pub stitches: u32,
    pub initial_stitches: u32,
    pub can_undo: bool,
    pub can_edit_rules: bool,
    pub can_complete: bool,
    pub pattern: Option<Pattern>,
    pub pattern_finished: bool,
    pub due_count: usize,
    pub pending_count: usize,
    pub projected_stitches: Option<u32>,
    pub segments: Page<SegmentItem>,
    pub reminders: Page<ReminderItem>,
    pub due: Page<DueItem>,
    pub history: Page<HistoryItem>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub revision: u32,
    pub projects: Page<ProjectItem>,
    pub parts: Page<PartItem>,
    pub active_project_id: Option<String>,
    pub project_name: Option<String>,
    pub part: Option<PartView>,
    pub pages: Pages,
}

pub fn part_view(part: &Part, pages: &Pages, ctx: &dyn TaskContext) -> PluginResult<PartView> {
    let next = part.completed + 1;
    let pattern = part
        .segments
        .iter()
        .enumerate()
        .find(|(_, s)| s.start <= next && next <= s.end())
        .map(|(index, s)| Pattern {
            id: s.id.clone(),
            name: s.name.clone(),
            index: index + 1,
            start: s.start,
            end: s.end(),
            row: (next - s.start) % s.rows + 1,
            repeat: (next - s.start) / s.rows + 1,
            repeats: s.repeats,
            rows: s.rows,
            completed_repeats: (next - s.start) / s.rows,
        });
    let segments = part
        .segments
        .iter()
        .enumerate()
        .map(|(index, s)| {
            let done = part
                .completed
                .saturating_add(1)
                .saturating_sub(s.start)
                .min(s.rows * s.repeats);
            SegmentItem {
                id: s.id.clone(),
                name: s.name.clone(),
                index: index + 1,
                start: s.start,
                end: s.end(),
                rows: s.rows,
                repeats: s.repeats,
                completed_repeats: done / s.rows,
                completed_rows: done,
            }
        })
        .collect();
    let mut actions: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    let mut history = Vec::with_capacity(part.undo.len());
    for (index, before) in part.undo.iter().enumerate() {
        if index.is_multiple_of(256) {
            check_cancel(ctx)?;
        }
        let (after_rows, after_stitches) = part
            .undo
            .get(index + 1)
            .map_or((part.completed, part.stitches), |s| {
                (s.completed, s.stitches)
            });
        let line = after_rows > before.completed;
        if line {
            for (id, choice) in &before.pending {
                let entry = actions.entry(id).or_default();
                match choice {
                    Choice::Execute => entry.0 += 1,
                    Choice::Skip => entry.1 += 1,
                }
            }
        }
        history.push(HistoryItem {
            index: index + 1,
            kind: if line { "line" } else { "adjust" },
            completed_before: before.completed,
            completed_after: after_rows,
            stitches_before: before.stitches,
            stitches_after: after_stitches,
        });
    }
    history.reverse();
    let reminders = part
        .reminders
        .iter()
        .enumerate()
        .map(|(index, r)| {
            let step = if next <= r.start {
                0
            } else {
                (next - r.start).div_ceil(r.interval)
            };
            let counts = actions.get(r.id.as_str()).copied().unwrap_or_default();
            ReminderItem {
                id: r.id.clone(),
                name: r.name.clone(),
                index: index + 1,
                start: r.start,
                interval: r.interval,
                repeats: r.repeats,
                delta: r.delta,
                next: (step < r.repeats).then(|| r.start + step * r.interval),
                executed: counts.0,
                skipped: counts.1,
            }
        })
        .collect();
    let due = part
        .reminders
        .iter()
        .enumerate()
        .filter_map(|(index, r)| {
            r.occurrence(next).map(|occurrence| DueItem {
                id: r.id.clone(),
                name: r.name.clone(),
                index: index + 1,
                delta: r.delta,
                occurrence,
                repeats: r.repeats,
                choice: part.pending.get(&r.id).copied(),
            })
        })
        .collect::<Vec<_>>();
    let pending_count = due.iter().filter(|r| r.choice.is_none()).count();
    let due_count = due.len();
    let projected_stitches = part.confirmed_stitches().ok();
    Ok(PartView {
        id: part.id.clone(),
        name: part.name.clone(),
        kind: part.kind,
        note: part.note.clone(),
        completed: part.completed,
        next_line: next,
        stitches: part.stitches,
        initial_stitches: part.initial_stitches,
        can_undo: !part.undo.is_empty(),
        can_edit_rules: part.completed == 0 && part.undo.is_empty(),
        can_complete: pending_count == 0
            && projected_stitches.is_some()
            && part.completed < MAX_ROWS,
        pattern,
        pattern_finished: part
            .segments
            .last()
            .is_some_and(|s| part.completed >= s.end()),
        due_count,
        pending_count,
        projected_stitches,
        segments: page(segments, pages.segments),
        reminders: page(reminders, pages.reminders),
        due: page(due, pages.due),
        history: page(history, pages.history),
    })
}

pub fn inspect(
    state: &Workshop,
    mut pages: Pages,
    follow: bool,
    ctx: &dyn TaskContext,
) -> PluginResult<View> {
    validate(state, ctx)?;
    let project = state
        .projects
        .iter()
        .find(|p| Some(&p.id) == state.active_project_id.as_ref());
    if follow {
        pages.projects = state
            .projects
            .iter()
            .position(|p| Some(&p.id) == state.active_project_id.as_ref())
            .unwrap_or_default()
            / PAGE_SIZE
            * PAGE_SIZE;
        pages.parts = project
            .and_then(|p| {
                p.parts
                    .iter()
                    .position(|part| Some(&part.id) == p.active_part_id.as_ref())
            })
            .unwrap_or_default()
            / PAGE_SIZE
            * PAGE_SIZE;
    }
    let projects = page(
        state
            .projects
            .iter()
            .map(|p| ProjectItem {
                id: p.id.clone(),
                name: p.name.clone(),
                part_count: p.parts.len(),
            })
            .collect(),
        pages.projects,
    );
    let parts = page(
        project
            .map(|p| {
                p.parts
                    .iter()
                    .map(|part| PartItem {
                        id: part.id.clone(),
                        name: part.name.clone(),
                        kind: part.kind,
                        completed: part.completed,
                        stitches: part.stitches,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        pages.parts,
    );
    let part = state
        .active()
        .map(|(_, part)| part_view(part, &pages, ctx))
        .transpose()?;
    pages.projects = projects.offset;
    pages.parts = parts.offset;
    if let Some(part) = &part {
        pages.segments = part.segments.offset;
        pages.reminders = part.reminders.offset;
        pages.due = part.due.offset;
        pages.history = part.history.offset;
    }
    check_cancel(ctx)?;
    Ok(View {
        revision: state.revision,
        projects,
        parts,
        active_project_id: state.active_project_id.clone(),
        project_name: project.map(|p| p.name.clone()),
        part,
        pages,
    })
}
