use super::*;
use std::sync::atomic::AtomicBool;
use tf_plugin_api::{LogLevel, PluginError};
#[derive(Default)]
struct Context {
    cancelled: AtomicBool,
    cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, s: &str) {
        if self.cancel_on == Some(s) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, p: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        std::fs::File::open(p)
            .map(|f| Box::new(f) as Box<dyn std::io::Read + Send>)
            .map_err(|_| PluginError::new("test.file"))
    }
    fn file_size(&self, p: &str) -> PluginResult<u64> {
        std::fs::metadata(p)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("test.file"))
    }
}
fn input(pricing: Value) -> Value {
    json!({"days":30,"currency":"CNY","appliances":[{"id":"a","name":"Heater","quantity":1,"usage":{"power":"1000","unit":"w","peak":"10","shoulder":"0","valley":"0"},"alternative":null}],"tariffs":[{"id":"t","name":"Custom","pricing":pricing}]})
}
fn flat() -> Value {
    input(json!({"kind":"flat","rate":"0.6"}))
}
fn calculate(v: Value) -> Report {
    engine::calculate(parse_args(v).unwrap(), &Context::default()).unwrap()
}
fn tiered() -> Value {
    input(
        json!({"kind":"tiered","tiers":[{"upTo":"100","rate":"0.5"},{"upTo":"200","rate":"0.8"},{"upTo":null,"rate":"1"}]}),
    )
}
#[test]
fn watt_and_kw_inputs_have_identical_exact_energy_and_cost() {
    let a = calculate(flat());
    let mut v = flat();
    v["appliances"][0]["usage"]["power"] = json!("1");
    v["appliances"][0]["usage"]["unit"] = json!("kw");
    let b = calculate(v);
    assert_eq!(a.bills[0].baseline.monthly_kwh, "300");
    assert_eq!(a.bills[0].baseline.monthly_cost, "180.00");
    assert_eq!(
        a.bills[0].baseline.monthly_cost,
        b.bills[0].baseline.monthly_cost
    );
    assert_eq!(a.bills[0].baseline.daily_cost, "6.00");
    assert_eq!(a.bills[0].baseline.annual_cost, "2160.00");
}
#[test]
fn monthly_tiers_are_progressive_and_never_restart_for_each_appliance() {
    let mut v = tiered();
    v["appliances"][0]["usage"]["peak"] = json!("5");
    let mut b = v["appliances"][0].clone();
    b["id"] = json!("b");
    v["appliances"].as_array_mut().unwrap().push(b);
    let r = calculate(v);
    let b = &r.bills[0];
    assert_eq!(b.baseline.monthly_cost, "230.00");
    assert_eq!(b.rows[0].baseline_cost, "115.00");
    assert_eq!(b.rows[1].baseline_cost, "115.00");
    assert_eq!(b.baseline.highest_tier, Some(2));
    assert_eq!(
        b.baseline
            .bands
            .iter()
            .map(|b| b.cost.as_str())
            .collect::<Vec<_>>(),
        vec!["50.00", "80.00", "100.00"]
    );
}
#[test]
fn exact_tier_boundary_does_not_claim_next_tier_was_used() {
    let mut v = tiered();
    v["days"] = json!(10);
    let r = calculate(v);
    assert_eq!(r.bills[0].baseline.highest_tier, Some(0));
    assert_eq!(r.bills[0].baseline.remaining_kwh.as_deref(), Some("0"));
    assert!(!r.bills[0].baseline.bands[1].reached);
}
#[test]
fn time_of_use_prices_only_the_assigned_period_energy() {
    let mut v = input(json!({"kind":"tou","peak":"1","shoulder":"0.5","valley":"0.2"}));
    v["appliances"][0]["usage"]["peak"] = json!("2");
    v["appliances"][0]["usage"]["shoulder"] = json!("3");
    v["appliances"][0]["usage"]["valley"] = json!("5");
    let r = calculate(v);
    assert_eq!(r.bills[0].baseline.monthly_cost, "135.00");
    assert_eq!(
        r.bills[0]
            .baseline
            .bands
            .iter()
            .map(|b| b.cost.as_str())
            .collect::<Vec<_>>(),
        vec!["60.00", "45.00", "30.00"]
    );
}
#[test]
fn alternatives_recompute_household_tiers_instead_of_using_old_marginal_price() {
    let mut v = tiered();
    let mut u = v["appliances"][0]["usage"].clone();
    u["power"] = json!("500");
    v["appliances"][0]["alternative"] = u;
    let r = calculate(v);
    let b = &r.bills[0];
    assert_eq!(b.alternative.monthly_cost, "90.00");
    assert_eq!(b.annual_saving, "1680.00");
    assert_eq!(b.saving_direction, "saving");
}
#[test]
fn more_power_reports_an_increase_and_missing_alternative_keeps_baseline() {
    let unchanged = calculate(flat());
    assert_eq!(unchanged.bills[0].saving_direction, "same");
    let mut v = flat();
    let mut u = v["appliances"][0]["usage"].clone();
    u["power"] = json!("2000");
    v["appliances"][0]["alternative"] = u;
    let r = calculate(v);
    assert_eq!(r.bills[0].saving_direction, "increase");
    assert_eq!(r.bills[0].annual_saving, "2160.00");
}
#[test]
fn zero_energy_and_free_tariffs_do_not_divide_by_zero() {
    let mut v = tiered();
    v["appliances"][0]["usage"]["power"] = json!("0");
    let r = calculate(v);
    assert_eq!(r.bills[0].baseline.monthly_cost, "0.00");
    assert_eq!(r.bills[0].baseline.highest_tier, None);
    assert_eq!(r.bills[0].rows[0].share_percent, "0.00");
    let mut v = flat();
    v["tariffs"][0]["pricing"]["rate"] = json!("0");
    assert_eq!(calculate(v).bills[0].baseline.monthly_cost, "0.00");
}
#[test]
fn one_final_half_up_round_and_largest_remainders_preserve_bill_cents() {
    let mut v = flat();
    v["days"] = json!(1);
    v["tariffs"][0]["pricing"]["rate"] = json!("5");
    v["appliances"][0]["usage"]["power"] = json!("1");
    v["appliances"][0]["usage"]["peak"] = json!("1");
    assert_eq!(calculate(v.clone()).bills[0].baseline.monthly_cost, "0.01");
    let mut b = v["appliances"][0].clone();
    b["id"] = json!("b");
    v["appliances"].as_array_mut().unwrap().push(b);
    let r = calculate(v);
    assert_eq!(r.bills[0].baseline.monthly_cost, "0.01");
    assert_eq!(r.bills[0].rows[0].baseline_cost, "0.01");
    assert_eq!(r.bills[0].rows[1].baseline_cost, "0.00");
}
#[test]
fn maximum_valid_input_does_not_overflow_allocation_products() {
    let mut v = flat();
    v["days"] = json!(31);
    v["tariffs"][0]["pricing"]["rate"] = json!("1000");
    let mut a = v["appliances"][0].clone();
    a["quantity"] = json!(1000);
    a["usage"]["power"] = json!("1000000");
    a["usage"]["peak"] = json!("24");
    v["appliances"] = Value::Array(
        (0..100)
            .map(|i| {
                let mut a = a.clone();
                a["id"] = json!(format!("a{i}"));
                a
            })
            .collect(),
    );
    let r = calculate(v);
    assert_eq!(r.bills[0].baseline.monthly_kwh, "74400000000");
    assert_eq!(r.bills[0].baseline.monthly_cost, "74400000000000.00");
    assert_eq!(r.bills[0].rows[99].share_percent, "1.00");
}
#[test]
fn runtime_totals_and_strict_decimal_limits_are_validated_in_rust() {
    for n in ["NaN", "1e3", "-1", " 1", "1.", ".5", "1.0000", "Infinity"] {
        let mut v = flat();
        v["appliances"][0]["usage"]["power"] = json!(n);
        assert!(
            engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err(),
            "{n}"
        );
    }
    let mut v = flat();
    v["appliances"][0]["usage"]["peak"] = json!("24");
    v["appliances"][0]["usage"]["valley"] = json!("0.001");
    assert!(engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err());
}
#[test]
fn malformed_or_unbounded_incomplete_tiers_are_rejected() {
    for tiers in [
        json!([]),
        json!([{"upTo":"100","rate":"1"}]),
        json!([{"upTo":null,"rate":"1"},{"upTo":null,"rate":"2"}]),
        json!([{"upTo":"100","rate":"1"},{"upTo":"90","rate":"2"},{"upTo":null,"rate":"3"}]),
    ] {
        let v = input(json!({"kind":"tiered","tiers":tiers}));
        assert!(engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err());
    }
}
#[test]
fn duplicate_ids_bad_names_and_invalid_period_budgets_are_rejected() {
    let mut v = flat();
    let a = v["appliances"][0].clone();
    v["appliances"].as_array_mut().unwrap().push(a);
    assert!(engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err());
    for days in [0, 32] {
        let mut v = flat();
        v["days"] = json!(days);
        assert!(engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err());
    }
    let mut v = flat();
    v["currency"] = json!("bad\n");
    assert!(engine::calculate(parse_args(v).unwrap(), &Context::default()).is_err());
}
#[test]
fn several_tariffs_share_energy_without_reusing_each_others_prices() {
    let mut v = flat();
    v["tariffs"]
        .as_array_mut()
        .unwrap()
        .push(tiered()["tariffs"][0].clone());
    v["tariffs"][1]["id"] = json!("tier");
    let r = calculate(v);
    assert_eq!(
        r.bills[0].baseline.monthly_kwh,
        r.bills[1].baseline.monthly_kwh
    );
    assert_eq!(r.bills[0].baseline.monthly_cost, "180.00");
    assert_eq!(r.bills[1].baseline.monthly_cost, "230.00");
}
#[test]
fn queries_are_paginated_and_old_handles_expire_only_after_success() {
    let p = ElectricityCost::default();
    let first = p
        .run_task("calculate", flat(), &Context::default())
        .unwrap();
    let h = first["handle"].clone();
    let q = json!({"handle":h,"tariff":0,"offset":0,"limit":1});
    assert_eq!(
        p.run_task("query", q.clone(), &Context::default()).unwrap()["total"],
        1
    );
    let cancelled = Context {
        cancel_on: Some("calculate"),
        ..Context::default()
    };
    assert_eq!(
        p.run_task("calculate", flat(), &cancelled)
            .unwrap_err()
            .code,
        "task.cancelled"
    );
    assert!(p.run_task("query", q.clone(), &Context::default()).is_ok());
    let mut bad = flat();
    bad["days"] = json!(0);
    assert!(p.run_task("calculate", bad, &Context::default()).is_err());
    assert!(p.run_task("query", q.clone(), &Context::default()).is_ok());
    p.run_task("calculate", flat(), &Context::default())
        .unwrap();
    assert_eq!(
        p.run_task("query", q, &Context::default())
            .unwrap_err()
            .code,
        "electricity-cost.expired"
    );
}
#[test]
fn query_rejects_invalid_tariff_and_page_sizes() {
    let p = ElectricityCost::default();
    let r = p
        .run_task("calculate", flat(), &Context::default())
        .unwrap();
    for (tariff, limit) in [(1, 10), (0, 0), (0, 101)] {
        assert!(
            p.run_task(
                "query",
                json!({"handle":r["handle"],"tariff":tariff,"limit":limit}),
                &Context::default()
            )
            .is_err()
        );
    }
}
#[test]
fn saved_tariffs_are_validated_paginated_and_updated_by_stable_id() {
    let p = ElectricityCost::default();
    let state = json!({"schemaVersion":1,"revision":0,"tariffs":[]});
    let tariff = flat()["tariffs"][0].clone();
    let a = p
        .call("book", json!({"state":state,"tariff":tariff}))
        .unwrap();
    let mut t = tariff.clone();
    t["name"] = json!("Edited");
    let b = p
        .call("book", json!({"state":a["state"],"tariff":t}))
        .unwrap();
    assert_eq!(b["state"]["revision"], 2);
    assert_eq!(b["page"]["total"], 1);
    assert_eq!(b["page"]["items"][0]["name"], "Edited");
    let c = p
        .call("book", json!({"state":b["state"],"remove":"t"}))
        .unwrap();
    assert_eq!(c["page"]["total"], 0);
}
#[test]
fn incompatible_books_and_duplicate_saved_ids_are_not_silently_replaced() {
    let p = ElectricityCost::default();
    assert!(
        p.call(
            "book",
            json!({"state":{"schemaVersion":2,"revision":0,"tariffs":[]}})
        )
        .is_err()
    );
    let t = flat()["tariffs"][0].clone();
    assert!(
        p.call(
            "book",
            json!({"state":{"schemaVersion":1,"revision":0,"tariffs":[t,t]}})
        )
        .is_err()
    );
}
#[test]
fn structured_agent_contract_has_no_external_listener_or_calculation_permissions() {
    let p = ElectricityCost::default();
    let s = p.call("schema", json!({})).unwrap();
    assert!(s["externalTransport"].is_null());
    assert_eq!(s["functions"]["calculate"]["permissions"], json!([]));
    assert_eq!(
        s["functions"]["calculate"]["inputSchema"]["properties"]["tariffs"]["maxItems"],
        8
    );
    let d = p.call("defaults", json!({})).unwrap();
    assert!(validate(&parse_args::<Input>(d).unwrap()).is_ok());
    assert!(parse_args::<Input>(json!({"days":30,"extra":true})).is_err());
}
#[test]
fn json_and_markdown_exports_include_actual_assumptions_and_escape_user_markup() {
    let p = ElectricityCost::default();
    let mut v = flat();
    v["appliances"][0]["name"] = json!("<script>*x*|[link]");
    let r = p.run_task("calculate", v, &Context::default()).unwrap();
    let a = p
        .run_task(
            "export",
            json!({"handle":r["handle"],"format":"json","language":"en-US"}),
            &Context::default(),
        )
        .unwrap();
    let value: Value = serde_json::from_str(a["content"].as_str().unwrap()).unwrap();
    assert_eq!(value["annualization"], "12 identical billing months");
    assert_eq!(value["bills"][0]["baseline"]["monthlyCost"], "180.00");
    let m = p
        .run_task(
            "export",
            json!({"handle":r["handle"],"format":"markdown","language":"zh-CN"}),
            &Context::default(),
        )
        .unwrap();
    assert!(
        m["content"]
            .as_str()
            .unwrap()
            .contains("&lt;script&gt;\\*x\\*\\|\\[link\\]")
    );
}
#[test]
fn cancellation_before_export_never_writes_a_file() {
    let p = ElectricityCost::default();
    let r = p
        .run_task("calculate", flat(), &Context::default())
        .unwrap();
    let path =
        std::env::temp_dir().join(format!("toolforge-energy-cancel-{}.md", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let c = Context {
        cancel_on: Some("export"),
        ..Context::default()
    };
    assert_eq!(
        p.run_task(
            "export",
            json!({"handle":r["handle"],"format":"markdown","language":"en-US","path":path}),
            &c
        )
        .unwrap_err()
        .code,
        "task.cancelled"
    );
    assert!(!path.exists());
}
