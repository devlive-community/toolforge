//! Chrome Trace Event JSON 后端：解析 X/B/E/M/I phase，按进程/线程汇总区间，
//! 诊断未闭合 B/E、负时长与非法事件。数量与输入设上限。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TraceViewer {
    manifest: Manifest,
}

impl Default for TraceViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TraceViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse_trace(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TraceViewer);

const MAX_EVENTS: usize = 20_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// Trace JSON 文本（顶层数组或 {traceEvents: [...]}）
    pub json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub name: String,
    pub phase: String,
    pub pid: i64,
    pub tid: i64,
    pub ts: f64,
    pub dur: Option<f64>,
    /// X 区间是否包含子区间（汇总时注明嵌套）
    pub has_children: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub name: String,
    pub count: u64,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub events: Vec<Event>,
    pub processes: Vec<ProcessInfo>,
    pub summaries: Vec<Summary>,
    pub unsupported_phases: Vec<String>,
    pub diagnostics: Vec<String>,
    pub total_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: i64,
    /// 进程名（M 元数据，缺省用 pid）
    pub name: String,
    pub threads: Vec<ThreadInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadInfo {
    pub tid: i64,
    pub name: String,
    pub events: usize,
}

fn num(value: &Value) -> Option<f64> {
    value.as_f64()
}

pub fn parse_trace(args: Args) -> PluginResult<Output> {
    if args.json.len() > 8 * 1024 * 1024 {
        return Err(PluginError::new("trace.too_large").with("max", 8 * 1024 * 1024));
    }
    let root: Value = serde_json::from_str(&args.json)
        .map_err(|e| PluginError::new("trace.bad_json").with("detail", e.to_string()))?;
    let events_value = match &root {
        Value::Array(items) => items.clone(),
        Value::Object(map) => match map.get("traceEvents") {
            Some(Value::Array(items)) => items.clone(),
            _ => return Err(PluginError::new("trace.no_events")),
        },
        _ => return Err(PluginError::new("trace.bad_json")),
    };
    if events_value.len() > MAX_EVENTS {
        return Err(PluginError::new("trace.too_many_events").with("max", MAX_EVENTS as u64));
    }
    let mut events: Vec<Event> = Vec::new();
    let mut open_b: std::collections::HashMap<(i64, i64), (String, f64)> =
        std::collections::HashMap::new();
    let mut diagnostics: Vec<String> = Vec::new();
    let mut unsupported_phases: Vec<String> = Vec::new();
    let mut proc_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut thread_names: std::collections::HashMap<(i64, i64), String> =
        std::collections::HashMap::new();

    for item in &events_value {
        let Some(map) = item.as_object() else {
            continue;
        };
        let name = map
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(unnamed)")
            .to_owned();
        let phase = map
            .get("ph")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        let pid = map.get("pid").and_then(num).unwrap_or(0.0) as i64;
        let tid = map.get("tid").and_then(num).unwrap_or(0.0) as i64;
        let ts = map.get("ts").and_then(num).unwrap_or(0.0);
        match phase.as_str() {
            "M" => {
                // 元数据：进程/线程名
                let arg_name = map
                    .get("args")
                    .and_then(|a| a.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if name == "process_name" {
                    proc_names.insert(pid, arg_name.to_owned());
                } else if name == "thread_name" {
                    thread_names.insert((pid, tid), arg_name.to_owned());
                }
            }
            "X" => {
                let dur = map.get("dur").and_then(num).unwrap_or(0.0);
                events.push(Event {
                    name,
                    phase: "X".into(),
                    pid,
                    tid,
                    ts,
                    dur: Some(dur),
                    has_children: false,
                });
            }
            "B" => {
                open_b.insert((pid, tid), (name.clone(), ts));
            }
            "E" => match open_b.remove(&(pid, tid)) {
                Some((b_name, b_ts)) => {
                    let dur = ts - b_ts;
                    if dur < 0.0 {
                        diagnostics.push(format!("negative_duration:{b_name}"));
                    }
                    events.push(Event {
                        name: b_name,
                        phase: "B".into(),
                        pid,
                        tid,
                        ts: b_ts,
                        dur: Some(dur),
                        has_children: false,
                    });
                }
                None => diagnostics.push(format!("unmatched_e:{name}")),
            },
            "I" | "i" => {
                events.push(Event {
                    name,
                    phase: "I".into(),
                    pid,
                    tid,
                    ts,
                    dur: None,
                    has_children: false,
                });
            }
            other => {
                let key = other.to_owned();
                if !unsupported_phases.contains(&key) {
                    unsupported_phases.push(key);
                }
            }
        }
    }
    if !open_b.is_empty() {
        diagnostics.push(format!("unclosed_b:{}", open_b.len()));
    }
    // 嵌套标记：X 事件同线程存在包含它的其他区间
    let x_events: Vec<(usize, f64, f64, i64)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, e)| e.dur.map(|d| (index, e.ts, e.ts + d, e.tid)))
        .collect();
    for (index, ts, end, tid) in &x_events {
        let nested = x_events.iter().any(|(other, ots, oend, otid)| {
            other != index
                && otid == tid
                && *ots <= *ts
                && *oend >= *end
                && (oend - ots) > (end - ts)
        });
        if nested {
            events[*index].has_children = true;
        }
    }
    // 进程/线程分组
    let mut pids: Vec<i64> = events.iter().map(|e| e.pid).collect();
    pids.sort_unstable();
    pids.dedup();
    let mut processes = Vec::new();
    for pid in pids {
        let mut tids: Vec<i64> = events
            .iter()
            .filter(|e| e.pid == pid)
            .map(|e| e.tid)
            .collect();
        tids.sort_unstable();
        tids.dedup();
        let threads = tids
            .iter()
            .map(|tid| ThreadInfo {
                tid: *tid,
                name: thread_names
                    .get(&(pid, *tid))
                    .cloned()
                    .unwrap_or_else(|| format!("thread-{tid}")),
                events: events
                    .iter()
                    .filter(|e| e.pid == pid && e.tid == *tid)
                    .count(),
            })
            .collect();
        processes.push(ProcessInfo {
            pid,
            name: proc_names
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| format!("process-{pid}")),
            threads,
        });
    }
    // 事件名汇总（仅 X 区间）
    let mut by_name: std::collections::HashMap<String, (u64, f64, f64)> =
        std::collections::HashMap::new();
    for event in &events {
        if let Some(dur) = event.dur {
            let entry = by_name.entry(event.name.clone()).or_insert((0, 0.0, 0.0));
            entry.0 += 1;
            entry.1 += dur;
            entry.2 = entry.2.max(dur);
        }
    }
    let mut summaries: Vec<Summary> = by_name
        .into_iter()
        .map(|(name, (count, total, max))| Summary {
            name,
            count,
            total_ms: total,
            max_ms: max,
        })
        .collect();
    summaries.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    diagnostics.truncate(20);
    Ok(Output {
        total_count: events_value.len() as u64,
        events,
        processes,
        summaries,
        unsupported_phases,
        diagnostics,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
