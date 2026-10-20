use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, to_value};

use crate::engine::{Output, workout_key};
use crate::model::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportArgs {
    pub head: Option<Head>,
    pub workouts: Vec<Workout>,
    pub format: String,
    pub offset: u64,
    pub directory: Option<String>,
    pub stamp: Option<String>,
}
fn cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
pub fn export(args: ExportArgs) -> PluginResult<Value> {
    let head = args.head.unwrap_or_else(Head::empty);
    validate_head(&head)?;
    if args.workouts.len() > 1 {
        return Err(error("batch"));
    }
    for w in &args.workouts {
        validate_workout(w)?;
    }
    let content = if args.format == "json" {
        serde_json::to_string_pretty(&json!({"format":"toolforge.workout-log","version":1,"head":head,"offset":args.offset,"workouts":args.workouts})).map_err(|_|error("backup"))?
    } else if args.format == "csv" {
        let mut text = String::from(
            "date,workout,exercise,equipment,mode,set,status,warmup,reps,weight,unit,note\r\n",
        );
        for w in &args.workouts {
            for e in &w.body.entries {
                for (i, s) in e.sets.iter().enumerate() {
                    let mode = match e.exercise.mode {
                        Mode::External => "external",
                        Mode::Bodyweight => "bodyweight",
                        Mode::Unknown => "unknown",
                    };
                    let status = match s.status {
                        Status::Pending => "pending",
                        Status::Done => "done",
                        Status::Skipped => "skipped",
                    };
                    let unit = if s.unit == Unit::Kg { "kg" } else { "lb" };
                    let fields = [
                        w.body.date.clone(),
                        w.body.name.clone(),
                        e.exercise.name.clone(),
                        e.exercise.equipment.clone(),
                        mode.into(),
                        (i + 1).to_string(),
                        status.into(),
                        s.warmup.to_string(),
                        s.reps.clone(),
                        s.weight.clone(),
                        unit.into(),
                        s.note.clone(),
                    ];
                    text.push_str(
                        &fields
                            .iter()
                            .map(|s| {
                                let safe = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                                    format!("'{s}")
                                } else {
                                    s.clone()
                                };
                                cell(&safe)
                            })
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                    text.push_str("\r\n");
                }
            }
        }
        text
    } else {
        return Err(error("format"));
    };
    if content.len() > 16_000_000 {
        return Err(error("batch"));
    }
    let filename = format!(
        "workout-log-{}-{:08}.{}",
        args.stamp
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or_else(|| jiff::Timestamp::now().as_millisecond().to_string()),
        args.offset,
        args.format
    );
    let path = args.directory.map(|directory| {
        std::path::Path::new(&directory)
            .join(&filename)
            .to_string_lossy()
            .into_owned()
    });
    Ok(json!({"content":content,"extension":args.format,"filename":filename,"path":path}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportArgs {
    pub text: String,
    pub head: Option<Head>,
    #[serde(default)]
    pub records: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    format: String,
    version: u8,
    head: Head,
    offset: u64,
    workouts: Vec<Workout>,
}
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub fn import(args: ImportArgs) -> PluginResult<Output> {
    if args.text.len() > 16_000_000 {
        return Err(error("backup"));
    }
    let backup: Backup = serde_json::from_str(&args.text).map_err(|_| error("backup"))?;
    if backup.format != "toolforge.workout-log" || backup.version != 1 || backup.workouts.len() > 1
    {
        return Err(error("backup"));
    }
    validate_head(&backup.head)?;
    let source = digest(&serde_json::to_vec(&backup.head).map_err(|_| error("backup"))?);
    let receipt = format!("import-{}", &digest(args.text.as_bytes())[..48]);
    if !args.records.contains_key(&receipt) {
        return Ok(Output {
            need: vec![receipt],
            ..Output::default()
        });
    }
    if args.records.get(&receipt) == Some(&Value::Bool(true)) {
        return Ok(Output {
            data: json!({"count":0,"offset":backup.offset,"total":backup.head.next_workout-1}),
            ..Output::default()
        });
    }
    let mut head = args.head.unwrap_or_else(Head::empty);
    validate_head(&head)?;
    if head.next_workout == 1 && head.exercises.is_empty() && head.templates.is_empty() {
        head = backup.head.clone();
        head.next_workout = 1;
        head.revision = 0;
        head.restore_source = Some(source.clone());
        for exercise in &mut head.exercises {
            exercise.history = 0;
        }
    } else if head.restore_source.as_deref() != Some(&source) {
        return Err(tf_plugin_api::PluginError::new("workout.importConflict"));
    }
    let mut records = args.records;
    let mut writes = BTreeMap::new();
    let mut need = Vec::new();
    for mut workout in backup.workouts.clone() {
        validate_workout(&workout)?;
        workout.id = head.next_workout;
        workout.undo = None;
        let day = crate::engine::day_key(&workout.body.date);
        let mut keys = vec![day];
        for entry in &workout.body.entries {
            let exercise = head
                .exercises
                .iter_mut()
                .find(|e| e.id == entry.exercise.id)
                .ok_or_else(|| error("exercise"))?;
            keys.push(crate::engine::index_key(
                &exercise.id,
                exercise.history / INDEX_SIZE,
            ));
            exercise.history += 1;
        }
        for key in keys {
            if !records.contains_key(&key) {
                need.push(key);
                continue;
            }
            let mut ids: Vec<u64> = if records[&key].is_null() {
                Vec::new()
            } else {
                serde_json::from_value(records[&key].clone()).map_err(|_| error("index"))?
            };
            ids.push(workout.id);
            let value = to_value(ids)?;
            records.insert(key.clone(), value.clone());
            writes.insert(key, value);
        }
        writes.insert(workout_key(workout.id), to_value(workout)?);
        head.next_workout += 1;
    }
    if !need.is_empty() {
        return Ok(Output {
            need,
            ..Output::default()
        });
    }
    head.revision += 1;
    validate_head(&head)?;
    writes.insert("head".into(), to_value(head)?);
    writes.insert(receipt, Value::Bool(true));
    Ok(Output {
        writes,
        data: json!({"count":backup.workouts.len(),"offset":backup.offset,"total":backup.head.next_workout-1}),
        ..Output::default()
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub writes: BTreeMap<String, Value>,
}
pub fn recover(args: Recovery) -> PluginResult<Value> {
    if args.writes.is_empty() || args.writes.len() > 128 {
        return Err(error("transaction"));
    }
    if serde_json::to_vec(&args.writes)
        .map_err(|_| error("transaction"))?
        .len()
        > 1_800_000
    {
        return Err(error("transaction"));
    }
    for (key, value) in &args.writes {
        if key == "head" {
            let h: Head = serde_json::from_value(value.clone()).map_err(|_| error("stored"))?;
            validate_head(&h)?;
        } else if key.starts_with("workout-") {
            let w: Workout = serde_json::from_value(value.clone()).map_err(|_| error("stored"))?;
            validate_workout(&w)?;
            if workout_key(w.id) != *key {
                return Err(error("id"));
            }
        } else if key.starts_with("import-") {
            if value != &Value::Bool(true) {
                return Err(error("transaction"));
            }
        } else if key.starts_with("index-exercise-") || key.starts_with("day-") {
            let ids: Vec<u64> =
                serde_json::from_value(value.clone()).map_err(|_| error("index"))?;
            if ids.iter().any(|v| *v == 0 || *v >= MAX_SAFE) || ids.windows(2).any(|p| p[0] >= p[1])
            {
                return Err(error("index"));
            }
            budget(&ids)?;
        } else {
            return Err(error("transaction"));
        }
    }
    Ok(json!({"writes":args.writes}))
}
#[cfg(test)]
#[path = "transfer_test.rs"]
mod tests;
