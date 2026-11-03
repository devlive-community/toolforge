use super::*;
use serde_json::json;

#[test]
fn lists_all_dynasties_in_order() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, 21);
    let keys: Vec<&str> = out.dynasties.iter().map(|d| d.key).collect();
    assert_eq!(keys[0], "xia");
    assert_eq!(keys[keys.len() - 1], "qing");
    // 时间线单调
    for pair in out.dynasties.windows(2) {
        assert!(
            pair[0].start <= pair[1].start,
            "{} <= {}",
            pair[0].key,
            pair[1].key
        );
    }
}

#[test]
fn filters_by_key() {
    let out = list(ListArgs {
        query: "song".into(),
    })
    .unwrap();
    assert_eq!(out.total, 2, "northern-song and southern-song");
    assert!(
        list(ListArgs {
            query: "不存在".into()
        })
        .is_err()
    );
}

#[test]
fn lookup_year_belongs_to_dynasty() {
    let out = lookup(LookupArgs { year: -221 }).unwrap();
    // 秦统一当年：战国按 -221 止、秦按 -221 起，两个断代并存
    assert_eq!(out.matches.len(), 2);
    assert!(out.matches.iter().any(|d| d.key == "qin"));
    assert!(out.bce);

    let out = lookup(LookupArgs { year: 742 }).unwrap();
    assert_eq!(out.matches.len(), 1);
    assert_eq!(out.matches[0].key, "tang");

    let out = lookup(LookupArgs { year: 1127 }).unwrap();
    assert_eq!(
        out.matches.len(),
        2,
        "northern-song ends and southern-song starts in 1127"
    );

    let out = lookup(LookupArgs { year: 2024 }).unwrap();
    assert!(out.matches.is_empty(), "modern years have no dynasty");
}

#[test]
fn rejects_invalid_years() {
    assert_eq!(
        lookup(LookupArgs { year: 0 }).unwrap_err().code,
        "dynasty.bad_year"
    );
    assert_eq!(
        lookup(LookupArgs { year: -9999 }).unwrap_err().code,
        "dynasty.bad_year"
    );
}

#[test]
fn dispatches_via_plugin() {
    let tool = DynastyTimeline::default();
    let out = tool.call("lookup", json!({"year": 1644})).unwrap();
    // 明亡清入关同一年，两个朝代都覆盖 1644
    assert_eq!(out["matches"].as_array().unwrap().len(), 2);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&DynastyTimeline::default()).unwrap();
    assert_eq!(checked, 2);
}
