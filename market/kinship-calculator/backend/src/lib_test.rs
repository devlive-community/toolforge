use super::*;
use serde_json::json;

#[test]
fn direct_and_extended_terms() {
    let cases: Vec<(&[&str], &str)> = vec![
        (&["f"], "爸爸"),
        (&["f", "f"], "爷爷"),
        (&["m", "f"], "外公/姥爷"),
        (&["f", "yb"], "叔叔"),
        (&["m", "ob"], "舅舅"),
        (&["f", "os", "s"], "表哥/表姐（姑表）"),
        (&["s", "w"], "儿媳"),
        (&["w", "f"], "岳父"),
        (&["ob", "s"], "侄子"),
        (&["s", "s"], "孙子"),
    ];
    for (chain, expected) in cases {
        let out = resolve(ResolveArgs {
            chain: chain.iter().map(|s| s.to_string()).collect(),
        })
        .unwrap();
        assert_eq!(out.term, expected, "chain {chain:?}");
    }
}

#[test]
fn description_and_reverse_data() {
    let out = resolve(ResolveArgs {
        chain: vec!["f".into(), "ob".into()],
    })
    .unwrap();
    assert_eq!(out.description, "爸爸 的 哥哥");
    assert_eq!(out.self_address, "侄子/侄女");
}

#[test]
fn rejects_bad_steps_and_unknown_chains() {
    let err = resolve(ResolveArgs {
        chain: vec!["x".into()],
    })
    .unwrap_err();
    assert_eq!(err.code, "kinship.bad_step");
    let err = resolve(ResolveArgs { chain: vec![] }).unwrap_err();
    assert_eq!(err.code, "kinship.empty_chain");
    // 过深或未覆盖的组合
    let err = resolve(ResolveArgs {
        chain: vec!["f".into(), "h".into(), "f".into(), "f".into(), "f".into()],
    })
    .unwrap_err();
    assert_eq!(err.code, "kinship.unknown_chain");
}

#[test]
fn dispatches_via_plugin() {
    let out = KinshipCalculator::default()
        .call("resolve", json!({"chain": ["f", "f"]}))
        .unwrap();
    assert_eq!(out["term"], "爷爷");
    assert_eq!(
        KinshipCalculator::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&KinshipCalculator::default()).unwrap();
    assert_eq!(checked, 1);
}
