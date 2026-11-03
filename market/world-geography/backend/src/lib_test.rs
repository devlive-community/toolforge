use super::*;
use serde_json::json;

#[test]
fn lists_all_and_searches_both_languages() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, COUNTRIES.len());
    assert_eq!(out.total, 63);

    let out = list(ListArgs {
        query: "中国".into(),
        continent: None,
    })
    .unwrap();
    assert_eq!(out.total, 1);
    assert_eq!(out.countries[0].code, "CN");

    let out = list(ListArgs {
        query: "tokyo".into(),
        continent: None,
    })
    .unwrap();
    assert_eq!(out.countries[0].code, "JP");

    let out = list(ListArgs {
        query: "+86".into(),
        continent: None,
    })
    .unwrap();
    assert_eq!(out.countries[0].code, "CN");

    let out = list(ListArgs {
        query: String::new(),
        continent: Some("europe".into()),
    })
    .unwrap();
    assert!(out.total >= 15);
    assert!(out.countries.iter().all(|c| c.continent == "europe"));
}

#[test]
fn capital_reverse_lookup() {
    let out = by_capital(CapitalArgs {
        capital: "巴黎".into(),
    })
    .unwrap();
    assert_eq!(out.countries[0].code, "FR");
    let out = by_capital(CapitalArgs {
        capital: "Singapore".into(),
    })
    .unwrap();
    assert_eq!(out.countries[0].code, "SG");
    assert!(
        by_capital(CapitalArgs {
            capital: "不存在".into()
        })
        .is_err()
    );
}

#[test]
fn dispatches_via_plugin() {
    let out = WorldGeography::default()
        .call("list", json!({"query": "Brazil"}))
        .unwrap();
    assert_eq!(out["countries"][0]["capitalEn"], "Brasília");
    assert_eq!(
        WorldGeography::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&WorldGeography::default()).unwrap();
    assert_eq!(checked, 2);
}
