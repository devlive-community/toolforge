use super::*;
use tf_plugin_api::PluginError;
use tf_plugin_api::ToolPlugin;

const SAMPLE: &str = r#"{
  "members": [
    {"id": "m1", "name": "甲"},
    {"id": "m2", "name": "乙"},
    {"id": "m3", "name": "丙"}
  ],
  "expenses": [
    {"id": "e1", "title": "车费", "date": "2026-10-01", "total": "200", "payer": "m1",
     "split": {"mode": "equal", "participants": ["m1", "m2", "m3"]}},
    {"id": "e2", "title": "餐费", "total": "90", "payer": "m2",
     "split": {"mode": "equal", "participants": ["m1", "m2"]}}
  ],
  "settlements": []
}"#;

fn settle_args(text: &str) -> Value {
    serde_json::json!({ "ledger": serde_json::from_str::<Value>(text).unwrap() })
}

#[test]
fn settle_returns_balances_and_transfers() {
    let plugin = ExpenseSplitter::default();
    let out = plugin.call("settle", settle_args(SAMPLE)).unwrap();
    assert_eq!(out["totalCents"], 29000);
    let balances = out["balances"].as_array().unwrap();
    assert_eq!(balances.len(), 3);
    // 甲：paid 20000，owed 6667+4500=11167 → net 8833
    assert_eq!(balances[0]["net"], 8833);
    // 乙：paid 9000，owed 6667+4500=11167 → net -2167
    assert_eq!(balances[1]["net"], -2167);
    // 丙：paid 0，owed 6666 → net -6666
    assert_eq!(balances[2]["net"], -6666);
    // 余额守恒
    let sum: i64 = balances.iter().map(|b| b["net"].as_i64().unwrap()).sum();
    assert_eq!(sum, 0);
    // 转账建议守恒：应收总额 = 甲的净应收
    let transfers = out["transfers"].as_array().unwrap();
    let transferred: i64 = transfers.iter().map(|t| t["cents"].as_i64().unwrap()).sum();
    assert_eq!(transferred, 8833);
}

#[test]
fn new_id_is_unique() {
    let plugin = ExpenseSplitter::default();
    let first = plugin.call("new_id", Value::Null).unwrap();
    let second = plugin.call("new_id", Value::Null).unwrap();
    assert_ne!(first["id"], second["id"]);
}

#[test]
fn invalid_ledger_returns_error_code() {
    let plugin = ExpenseSplitter::default();
    let err = plugin
        .call(
            "settle",
            settle_args(r#"{"members": [], "expenses": [], "settlements": []}"#),
        )
        .unwrap_err();
    assert_eq!(err.code, "esp.no_members");
}

#[test]
fn export_requires_report_and_renders_formats() {
    let plugin = ExpenseSplitter::default();
    let settled = plugin.call("settle", settle_args(SAMPLE)).unwrap();
    // CSV
    let out = plugin
        .call(
            "export",
            serde_json::json!({"format": "csv", "report": settled}),
        )
        .unwrap();
    assert_eq!(out["ext"], "csv");
    let csv = out["content"].as_str().unwrap();
    assert!(csv.starts_with("type,id,name,value"));
    assert!(csv.contains("balance,m1,甲,200.00,111.67,0.00,0.00,88.33"));
    // 文本
    let out = plugin
        .call(
            "export",
            serde_json::json!({"format": "text", "report": settled}),
        )
        .unwrap();
    assert!(
        out["content"]
            .as_str()
            .unwrap()
            .contains("Suggested transfers")
    );
    // JSON 往返
    let out = plugin
        .call(
            "export",
            serde_json::json!({"format": "json", "report": settled}),
        )
        .unwrap();
    let parsed: Value = serde_json::from_str(out["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed["totalCents"], 29000);
}

#[test]
fn actual_settlements_are_preserved_in_all_export_formats() {
    let plugin = ExpenseSplitter::default();
    let mut args = settle_args(SAMPLE);
    args["ledger"]["settlements"] = serde_json::json!([
        {"id": "s,\"1", "from": "m2", "to": "m1", "cents": "12.34", "date": "2026-10-20"},
        {"id": "s2", "from": "m3", "to": "m1", "cents": "0.01"}
    ]);
    let report = plugin.call("settle", args).unwrap();
    let output = |format: &str| {
        plugin
            .call(
                "export",
                serde_json::json!({"format": format, "report": report}),
            )
            .unwrap()["content"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let parsed: Value = serde_json::from_str(&output("json")).unwrap();
    assert_eq!(parsed["settlements"][0]["id"], "s,\"1");
    assert_eq!(parsed["settlements"][0]["from"], "m2");
    assert_eq!(parsed["settlements"][0]["to"], "m1");
    assert_eq!(parsed["settlements"][0]["cents"], 1234);
    assert_eq!(parsed["settlements"][0]["date"], "2026-10-20");
    assert_eq!(parsed["settlements"][1]["cents"], 1);
    assert!(parsed["settlements"][1].get("date").is_none());
    let csv = output("csv");
    assert!(csv.contains("settlement,\"s,\"\"1\",m2,m1,12.34,2026-10-20\n"));
    assert!(csv.contains("settlement,s2,m3,m1,0.01,\n"));
    let text = output("text");
    assert!(text.contains("Actual settlements:\n"));
    assert!(text.contains("- s,\"1 (2026-10-20): 乙 -> 甲: 12.34\n"));
    assert!(text.contains("- s2: 丙 -> 甲: 0.01\n"));
}

#[test]
fn legacy_reports_without_settlement_details_remain_exportable() {
    let plugin = ExpenseSplitter::default();
    let mut report = plugin.call("settle", settle_args(SAMPLE)).unwrap();
    report.as_object_mut().unwrap().remove("settlements");
    for format in ["csv", "text", "json"] {
        let exported = plugin
            .call(
                "export",
                serde_json::json!({"format": format, "report": report}),
            )
            .unwrap();
        assert!(!exported["content"].as_str().unwrap().is_empty());
        if format == "json" {
            let parsed: Value =
                serde_json::from_str(exported["content"].as_str().unwrap()).unwrap();
            assert_eq!(parsed["settlements"], serde_json::json!([]));
        }
    }
}

#[test]
fn settle_validation_errors_carry_params() {
    let plugin = ExpenseSplitter::default();
    let err: PluginError = plugin
        .call(
            "settle",
            settle_args(r#"{"members": [{"id": "m1", "name": "甲"}], "expenses": [{"id": "e1", "title": "", "total": "abc", "payer": "m1", "split": {"mode": "equal", "participants": ["m1"]}}], "settlements": []}"#),
        )
        .unwrap_err();
    assert_eq!(err.code, "esp.bad_total");
}

#[test]
fn unknown_function_is_rejected() {
    let err = ExpenseSplitter::default()
        .call("send", Value::Null)
        .unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
