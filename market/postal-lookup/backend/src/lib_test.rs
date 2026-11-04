use super::*;
use serde_json::json;

#[test]
fn lookups_in_all_directions() {
    // 城市
    let out = lookup(Args {
        query: "北京".into(),
    })
    .unwrap();
    assert_eq!(out.results[0].postal_code, "100000");
    assert_eq!(out.results[0].area_code, "010");
    assert_eq!(out.results[0].admin_code, "110000");
    // 英文名
    let out = lookup(Args {
        query: "shanghai".into(),
    })
    .unwrap();
    assert_eq!(out.results[0].city_zh, "上海");
    // 邮编
    let out = lookup(Args {
        query: "518000".into(),
    })
    .unwrap();
    assert_eq!(out.results[0].city_zh, "深圳");
    assert_eq!(out.results[0].matched_by, "postal");
    // 区号
    let out = lookup(Args {
        query: "028".into(),
    })
    .unwrap();
    assert_eq!(out.results[0].city_zh, "成都");
    assert_eq!(out.results[0].matched_by, "area");
    // 别名
    let out = lookup(Args {
        query: "春城".into(),
    })
    .unwrap();
    assert_eq!(out.results[0].city_zh, "昆明");
}

#[test]
fn rejects_empty_and_unknown() {
    assert_eq!(
        lookup(Args {
            query: String::new()
        })
        .unwrap_err()
        .code,
        "postal.empty_query"
    );
    assert_eq!(
        lookup(Args {
            query: "999999".into()
        })
        .unwrap_err()
        .code,
        "postal.not_found"
    );
}

#[test]
fn dispatches_via_plugin() {
    let out = PostalLookup::default()
        .call("lookup", json!({"query": "北京"}))
        .unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(
        PostalLookup::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PostalLookup::default()).unwrap();
    assert_eq!(checked, 1);
}
