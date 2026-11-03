use super::*;
use serde_json::json;

const TRACE: &str = r#"{"traceEvents":[
  {"name":"process_name","ph":"M","pid":1,"tid":0,"args":{"name":"main-process"}},
  {"name":"thread_name","ph":"M","pid":1,"tid":11,"args":{"name":"worker"}},
  {"name":"render","ph":"X","pid":1,"tid":11,"ts":100,"dur":50},
  {"name":"layout","ph":"X","pid":1,"tid":11,"ts":110,"dur":30},
  {"name":"begin","ph":"B","pid":1,"tid":12,"ts":120},
  {"name":"end","ph":"E","pid":1,"tid":12,"ts":160},
  {"name":"instant","ph":"I","pid":1,"tid":11,"ts":180},
  {"name":"weird","ph":"S","pid":1,"tid":11,"ts":200},
  {"name":"orphan","ph":"E","pid":1,"tid":13,"ts":210}
]}"#;

fn parse_sample() -> Output {
    parse_trace(Args { json: TRACE.into() }).unwrap()
}

#[test]
fn parses_x_b_e_i_and_metadata() {
    let out = parse_sample();
    assert_eq!(out.events.len(), 4, "X2 + one B/E pair + I");
    assert_eq!(out.processes.len(), 1);
    assert_eq!(out.processes[0].name, "main-process");
    let worker = out.processes[0]
        .threads
        .iter()
        .find(|t| t.tid == 11)
        .unwrap();
    assert_eq!(worker.name, "worker");
    // B/E 配对区间
    let begin = out.events.iter().find(|e| e.name == "begin").unwrap();
    assert_eq!(begin.dur, Some(40.0));
}

#[test]
fn detects_nesting_and_diagnostics() {
    let out = parse_sample();
    let layout = out.events.iter().find(|e| e.name == "layout").unwrap();
    assert!(layout.has_children, "layout nests inside render");
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.starts_with("unmatched_e:orphan"))
    );
    assert!(out.unsupported_phases.contains(&"S".to_string()));
}

#[test]
fn summaries_total_by_name() {
    let out = parse_sample();
    let layout = out.summaries.iter().find(|s| s.name == "layout").unwrap();
    assert_eq!(layout.count, 1);
    assert_eq!(layout.max_ms, 30.0);
}

#[test]
fn rejects_bad_input() {
    let err = parse_trace(Args {
        json: "{\"name\":\"demo\"}".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "trace.no_events");
    let err = parse_trace(Args {
        json: "not json".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "trace.bad_json");
}

#[test]
fn dispatches_via_plugin() {
    let out = TraceViewer::default().call("parse", json!({"json": "[{\"name\":\"a\",\"ph\":\"X\",\"pid\":1,\"tid\":1,\"ts\":0,\"dur\":5}]"})).unwrap();
    assert_eq!(out["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        TraceViewer::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TraceViewer::default()).unwrap();
    assert_eq!(checked, 1);
}
