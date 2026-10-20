use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{PluginError, PluginResult, to_value};

use crate::model::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    pub head: Option<Head>,
    #[serde(default)]
    pub records: BTreeMap<String, Value>,
    pub action: Action,
}
#[derive(Clone, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Home {
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        template_offset: Option<usize>,
        #[serde(default = "ten")]
        limit: usize,
    },
    Exercise {
        id: Option<String>,
        name: String,
        equipment: String,
        mode: Mode,
    },
    Archive {
        id: String,
    },
    Start {
        name: String,
        date: String,
        template: Option<String>,
    },
    Get {
        workout: u64,
        #[serde(default)]
        warmups: bool,
    },
    Details {
        workout: u64,
        name: String,
        date: String,
        note: String,
    },
    AddExercise {
        workout: u64,
        exercise: String,
    },
    AddSet {
        workout: u64,
        entry: usize,
        copy: bool,
    },
    Set {
        workout: u64,
        entry: usize,
        index: usize,
        value: Set,
    },
    Undo {
        workout: u64,
    },
    Finish {
        workout: u64,
    },
    Rest {
        workout: u64,
        seconds: String,
        stop: bool,
    },
    SaveTemplate {
        workout: u64,
        id: Option<String>,
        name: String,
    },
    DeleteTemplate {
        id: String,
    },
    List {
        #[serde(default)]
        offset: usize,
        #[serde(default = "ten")]
        limit: usize,
        exercise: Option<String>,
        date: Option<String>,
        #[serde(default)]
        warmups: bool,
    },
}
fn ten() -> usize {
    10
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub need: Vec<String>,
    pub writes: BTreeMap<String, Value>,
    pub data: Value,
}
pub fn workout_key(id: u64) -> String {
    format!("workout-{id}")
}
pub fn index_key(id: &str, page: u64) -> String {
    format!("index-{id}-{page}")
}
pub fn day_key(day: &str) -> String {
    format!("day-{day}")
}
fn need(keys: Vec<String>) -> Output {
    Output {
        need: keys,
        ..Output::default()
    }
}
fn read<T: serde::de::DeserializeOwned>(
    records: &BTreeMap<String, Value>,
    key: &str,
) -> PluginResult<T> {
    serde_json::from_value(records.get(key).cloned().ok_or_else(|| error("missing"))?)
        .map_err(|_| error("stored"))
}
fn ids(records: &BTreeMap<String, Value>, key: &str) -> PluginResult<Vec<u64>> {
    if records.get(key).is_some_and(Value::is_null) {
        return Ok(Vec::new());
    }
    let list: Vec<u64> = read(records, key)?;
    if list.iter().any(|v| *v == 0 || *v >= MAX_SAFE)
        || list.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(error("index"));
    }
    budget(&list)?;
    Ok(list)
}
fn page(offset: usize, limit: usize) -> PluginResult<()> {
    if limit == 0 || limit > 20 || offset > MAX_SAFE as usize {
        return Err(error("page"));
    }
    Ok(())
}
fn entry(workout: &mut Workout, index: usize) -> PluginResult<&mut Entry> {
    workout
        .body
        .entries
        .get_mut(index)
        .ok_or_else(|| error("entry"))
}
fn lookup(head: &Head, id: &str) -> PluginResult<Exercise> {
    head.exercises
        .iter()
        .find(|e| e.id == id && !e.archived)
        .cloned()
        .ok_or_else(|| error("exercise"))
}
pub fn summary(workout: &Workout, warmups: bool) -> PluginResult<Value> {
    let mut completed = 0u64;
    let mut pending = 0u64;
    let mut reps = 0u64;
    let mut entries = Vec::new();
    for e in &workout.body.entries {
        let mut sets = Vec::new();
        for (index, s) in e.sets.iter().enumerate() {
            if s.status == Status::Pending {
                pending += 1;
            }
            if s.status != Status::Done || (s.warmup && !warmups) {
                continue;
            }
            completed += 1;
            reps += repetitions(&s.reps)?.unwrap_or(0) as u64;
            sets.push(json!({"index":index,"reps":s.reps,"weight":s.weight,"unit":s.unit,"warmup":s.warmup,"kg":kilograms(s)?.map(|n| format!("{n:.3}")),"note":s.note}));
        }
        entries.push(json!({"exercise":e.exercise,"sets":sets}));
    }
    Ok(json!({"completed":completed,"pending":pending,"reps":reps,"entries":entries}))
}
fn home(head: &Head, offset: usize, template_offset: usize, limit: usize) -> Value {
    json!({"exercises":head.exercises.iter().skip(offset).take(limit).collect::<Vec<_>>(),"exerciseTotal":head.exercises.len(),
        "templates":head.templates.iter().skip(template_offset).take(limit).collect::<Vec<_>>(),"templateTotal":head.templates.len(),
        "historyChoices":head.exercises.iter().map(|e|json!({"id":e.id,"name":e.name,"equipment":e.equipment,"archived":e.archived})).collect::<Vec<_>>(),
        "choices":head.exercises.iter().filter(|e|!e.archived).collect::<Vec<_>>(),
        "templateChoices":head.templates.iter().map(|t|json!({"id":t.id,"name":t.name})).collect::<Vec<_>>(),
        "workoutTotal":head.next_workout-1,"today":jiff::Zoned::now().date().to_string()})
}
fn finish(mut head: Head, mut out: Output) -> PluginResult<Output> {
    if !out.writes.is_empty() {
        head.restore_source = None;
        head.revision = head
            .revision
            .checked_add(1)
            .filter(|n| *n < MAX_SAFE)
            .ok_or_else(|| error("revision"))?;
        validate_head(&head)?;
        out.writes.insert("head".into(), to_value(&head)?);
    }
    Ok(out)
}
fn write_index(
    records: &BTreeMap<String, Value>,
    writes: &mut BTreeMap<String, Value>,
    key: &str,
    id: u64,
    add: bool,
) -> PluginResult<()> {
    let mut list = ids(records, key)?;
    if add && !list.contains(&id) {
        list.push(id);
        list.sort_unstable();
    }
    if !add {
        list.retain(|v| *v != id);
    }
    budget(&list)?;
    writes.insert(key.to_owned(), to_value(list)?);
    Ok(())
}
fn list(
    head: &Head,
    records: &BTreeMap<String, Value>,
    offset: usize,
    limit: usize,
    exercise: Option<String>,
    date: Option<String>,
    warmups: bool,
) -> PluginResult<Output> {
    page(offset, limit)?;
    let mut selected = Vec::new();
    let total;
    if let Some(day) = date.filter(|s| !s.is_empty()) {
        crate::model::date(&day)?;
        let key = day_key(&day);
        if !records.contains_key(&key) {
            return Ok(need(vec![key]));
        }
        let values = ids(records, &key)?;
        total = values.len() as u64;
        selected.extend(values.into_iter().rev().skip(offset).take(limit));
    } else if let Some(exercise) = exercise.filter(|s| !s.is_empty()) {
        let e = head
            .exercises
            .iter()
            .find(|e| e.id == exercise)
            .ok_or_else(|| error("exercise"))?;
        total = e.history;
        let ordinals: Vec<u64> = (0..limit as u64)
            .filter_map(|i| total.checked_sub(offset as u64 + i + 1))
            .collect();
        let missing: Vec<String> = ordinals
            .iter()
            .map(|n| index_key(&exercise, n / INDEX_SIZE))
            .filter(|k| !records.contains_key(k))
            .collect();
        if !missing.is_empty() {
            return Ok(need(missing));
        }
        for ordinal in ordinals {
            let values = ids(records, &index_key(&exercise, ordinal / INDEX_SIZE))?;
            selected.push(
                *values
                    .get((ordinal % INDEX_SIZE) as usize)
                    .ok_or_else(|| error("index"))?,
            );
        }
    } else {
        total = head.next_workout - 1;
        selected.extend(
            (0..limit as u64)
                .filter_map(|i| total.checked_sub(offset as u64 + i))
                .filter(|n| *n > 0),
        );
    }
    let missing: Vec<_> = selected
        .iter()
        .map(|n| workout_key(*n))
        .filter(|k| !records.contains_key(k))
        .collect();
    if !missing.is_empty() {
        return Ok(need(missing));
    }
    let mut rows = Vec::new();
    for id in selected {
        let w: Workout = read(records, &workout_key(id))?;
        validate_workout(&w)?;
        rows.push(json!({"workout":w,"summary":summary(&w,warmups)?}));
    }
    Ok(Output {
        data: json!({"rows":rows,"total":total,"offset":offset,"limit":limit}),
        ..Output::default()
    })
}
pub fn plan(args: Args) -> PluginResult<Output> {
    let mut head = args.head.unwrap_or_else(Head::empty);
    validate_head(&head)?;
    let records = args.records;
    let action = args.action;
    let mut out = Output::default();
    match action.clone() {
        Action::Home {
            offset,
            template_offset,
            limit,
        } => {
            page(offset, limit)?;
            let template_offset = template_offset.unwrap_or(offset);
            page(template_offset, limit)?;
            out.data = home(&head, offset, template_offset, limit);
            return Ok(out);
        }
        Action::List {
            offset,
            limit,
            exercise,
            date,
            warmups,
        } => return list(&head, &records, offset, limit, exercise, date, warmups),
        Action::Exercise {
            id,
            name,
            equipment,
            mode,
        } => {
            text(&name, "name", 100, true)?;
            text(&equipment, "equipment", 120, false)?;
            if let Some(id) = id {
                let e = head
                    .exercises
                    .iter_mut()
                    .find(|e| e.id == id)
                    .ok_or_else(|| error("exercise"))?;
                e.name = name.trim().into();
                e.equipment = equipment.trim().into();
                e.mode = mode;
            } else {
                let id = head.id("exercise")?;
                head.exercises.push(Exercise {
                    id,
                    name: name.trim().into(),
                    equipment: equipment.trim().into(),
                    mode,
                    archived: false,
                    history: 0,
                });
            }
            out.writes.insert("head".into(), Value::Null);
        }
        Action::Archive { id } => {
            let e = head
                .exercises
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or_else(|| error("exercise"))?;
            e.archived = !e.archived;
            out.writes.insert("head".into(), Value::Null);
        }
        Action::DeleteTemplate { id } => {
            head.templates.retain(|t| t.id != id);
            out.writes.insert("head".into(), Value::Null);
        }
        Action::Start {
            name,
            date,
            template,
        } => {
            text(&name, "name", 100, true)?;
            crate::model::date(&date)?;
            let id = head.next_workout;
            if id >= MAX_SAFE {
                return Err(error("sequence"));
            }
            let mut entries = if let Some(t) = template.filter(|t| !t.is_empty()) {
                head.templates
                    .iter()
                    .find(|v| v.id == t)
                    .ok_or_else(|| error("template"))?
                    .entries
                    .clone()
            } else {
                Vec::new()
            };
            let day = day_key(&date);
            let mut required = vec![day.clone()];
            for e in &entries {
                let original = head
                    .exercises
                    .iter()
                    .find(|v| v.id == e.exercise.id)
                    .ok_or_else(|| error("exercise"))?;
                required.push(index_key(&original.id, original.history / INDEX_SIZE));
            }
            let missing: Vec<_> = required
                .into_iter()
                .filter(|k| !records.contains_key(k))
                .collect();
            if !missing.is_empty() {
                return Ok(need(missing));
            }
            for e in &mut entries {
                for s in &mut e.sets {
                    s.status = Status::Pending;
                }
                let original = head
                    .exercises
                    .iter_mut()
                    .find(|v| v.id == e.exercise.id)
                    .ok_or_else(|| error("exercise"))?;
                write_index(
                    &records,
                    &mut out.writes,
                    &index_key(&original.id, original.history / INDEX_SIZE),
                    id,
                    true,
                )?;
                original.history += 1;
            }
            let w = Workout {
                id,
                body: Body {
                    name,
                    date,
                    note: String::new(),
                    finished: false,
                    entries,
                    rest: None,
                },
                undo: None,
            };
            validate_workout(&w)?;
            write_index(&records, &mut out.writes, &day, id, true)?;
            out.writes.insert(workout_key(id), to_value(&w)?);
            head.next_workout += 1;
            out.data = json!({"workout":w,"summary":summary(&w,false)?});
        }
        other => {
            let id = match &other {
                Action::Get { workout, .. }
                | Action::Details { workout, .. }
                | Action::AddExercise { workout, .. }
                | Action::AddSet { workout, .. }
                | Action::Set { workout, .. }
                | Action::Undo { workout }
                | Action::Finish { workout }
                | Action::Rest { workout, .. }
                | Action::SaveTemplate { workout, .. } => *workout,
                _ => return Err(error("operation")),
            };
            if id == 0 || id >= head.next_workout {
                return Err(error("workout"));
            }
            let key = workout_key(id);
            if !records.contains_key(&key) {
                return Ok(need(vec![key]));
            }
            let mut w: Workout = read(&records, &key)?;
            validate_workout(&w)?;
            let before = w.clone();
            match other {
                Action::Get { warmups, .. } => {
                    out.data = json!({"workout":w,"summary":summary(&w,warmups)?});
                    return Ok(out);
                }
                Action::Details {
                    name, date, note, ..
                } => {
                    text(&name, "name", 100, true)?;
                    crate::model::date(&date)?;
                    text(&note, "note", 4000, false)?;
                    if date != w.body.date {
                        let keys = [day_key(&w.body.date), day_key(&date)];
                        let missing: Vec<_> = keys
                            .iter()
                            .filter(|k| !records.contains_key(*k))
                            .cloned()
                            .collect();
                        if !missing.is_empty() {
                            return Ok(need(missing));
                        }
                        write_index(&records, &mut out.writes, &keys[0], id, false)?;
                        write_index(&records, &mut out.writes, &keys[1], id, true)?;
                    }
                    w.body.name = name;
                    w.body.date = date;
                    w.body.note = note;
                    w.undo = None;
                }
                Action::AddExercise { exercise, .. } => {
                    if w.body.entries.iter().any(|e| e.exercise.id == exercise) {
                        return Err(PluginError::new("workout.duplicateExercise"));
                    }
                    let e = lookup(&head, &exercise)?;
                    let index = index_key(&e.id, e.history / INDEX_SIZE);
                    if !records.contains_key(&index) {
                        return Ok(need(vec![index]));
                    }
                    write_index(&records, &mut out.writes, &index, id, true)?;
                    head.exercises
                        .iter_mut()
                        .find(|v| v.id == exercise)
                        .ok_or_else(|| error("exercise"))?
                        .history += 1;
                    w.body.entries.push(Entry {
                        exercise: e,
                        sets: vec![Set::default()],
                    });
                    w.undo = None;
                }
                Action::AddSet {
                    entry: index, copy, ..
                } => {
                    w.remember();
                    let e = entry(&mut w, index)?;
                    let mut set = if copy {
                        e.sets.last().cloned().unwrap_or_default()
                    } else {
                        Set::default()
                    };
                    set.status = Status::Pending;
                    e.sets.push(set);
                }
                Action::Set {
                    entry: index,
                    index: set,
                    value,
                    ..
                } => {
                    let e = w.body.entries.get(index).ok_or_else(|| error("entry"))?;
                    validate_set(&value, e.exercise.mode)?;
                    if e.sets.get(set) != Some(&value) {
                        w.remember();
                        let e = entry(&mut w, index)?;
                        *e.sets.get_mut(set).ok_or_else(|| error("set"))? = value;
                    }
                }
                Action::Undo { .. } => {
                    w.body = w
                        .undo
                        .take()
                        .ok_or_else(|| PluginError::new("workout.noUndo"))?;
                }
                Action::Finish { .. } => {
                    if !w.body.finished {
                        w.remember();
                        w.body.finished = true;
                        w.body.rest = None;
                    }
                }
                Action::Rest { seconds, stop, .. } => {
                    let seconds = seconds.parse::<u32>().map_err(|_| error("rest"))?;
                    if !stop && (seconds == 0 || seconds > 3600) {
                        return Err(error("rest"));
                    }
                    if stop {
                        if let Some(r) = &mut w.body.rest {
                            r.stopped = true;
                        }
                    } else {
                        w.body.rest = Some(Rest {
                            started: jiff::Timestamp::now().as_second(),
                            seconds,
                            stopped: false,
                        });
                    }
                }
                Action::SaveTemplate { id, name, .. } => {
                    text(&name, "name", 100, true)?;
                    if w.body.entries.is_empty() {
                        return Err(error("exercises"));
                    }
                    let mut entries = w.body.entries.clone();
                    for e in &mut entries {
                        for s in &mut e.sets {
                            s.status = Status::Pending;
                        }
                    }
                    if let Some(id) = id {
                        let t = head
                            .templates
                            .iter_mut()
                            .find(|t| t.id == id)
                            .ok_or_else(|| error("template"))?;
                        t.name = name;
                        t.entries = entries;
                    } else {
                        let id = head.id("template")?;
                        head.templates.push(Template { id, name, entries });
                    }
                    out.writes.insert("head".into(), Value::Null);
                }
                _ => return Err(error("operation")),
            }
            validate_workout(&w)?;
            if w != before {
                out.writes.insert(key, to_value(&w)?);
            }
            out.data = json!({"workout":w,"summary":summary(&w,false)?});
        }
    }
    if out.data.is_null() {
        out.data = home(&head, 0, 0, 10);
    }
    finish(head, out)
}
#[cfg(test)]
#[path = "engine_test.rs"]
mod tests;
