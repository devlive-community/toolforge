use super::*;
use serde_json::json;

#[test]
fn searches_both_languages_and_symbols() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, ELEMENTS.len());
    let out = list(ListArgs {
        query: "铁".into(),
        category: None,
    })
    .unwrap();
    assert_eq!(out.elements[0].symbol, "Fe");
    let out = list(ListArgs {
        query: "iron".into(),
        category: None,
    })
    .unwrap();
    assert_eq!(out.elements[0].symbol, "Fe");
    let out = list(ListArgs {
        query: "26".into(),
        category: None,
    })
    .unwrap();
    assert_eq!(out.elements[0].symbol, "Fe");
    let out = list(ListArgs {
        query: String::new(),
        category: Some("noble".into()),
    })
    .unwrap();
    assert_eq!(out.total, 3, "He Ne Ar in the sample data");
    assert!(
        list(ListArgs {
            query: "不存在".into(),
            category: None
        })
        .is_err()
    );
}

#[test]
fn molar_mass_water_and_copper_sulfate() {
    let out = molar_mass(MolarArgs {
        formula: "H2O".into(),
    })
    .unwrap();
    assert!(
        (out.molar_mass - 18.015).abs() < 0.01,
        "got {}",
        out.molar_mass
    );
    assert_eq!(out.parts.len(), 2);
    let out = molar_mass(MolarArgs {
        formula: "CuSO4".into(),
    })
    .unwrap();
    // Cu 63.546 + S 32.06 + O4 63.996 = 159.602
    assert!(
        (out.molar_mass - 159.602).abs() < 0.01,
        "got {}",
        out.molar_mass
    );
    assert_eq!(out.parts.iter().find(|p| p.symbol == "O").unwrap().count, 4);
}

#[test]
fn formula_errors() {
    let err = molar_mass(MolarArgs {
        formula: "h2o".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "element.bad_formula");
    let err = molar_mass(MolarArgs {
        formula: "Xx".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "element.unknown_symbol");
    let err = molar_mass(MolarArgs {
        formula: "H0".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "element.bad_count");
}

#[test]
fn dispatches_via_plugin() {
    let tool = PeriodicTable::default();
    let out = tool.call("list", json!({"query": "氢"})).unwrap();
    assert_eq!(out["elements"][0]["symbol"], "H");
    let out = tool.call("molar_mass", json!({"formula": "H2O"})).unwrap();
    assert!(out["molarMass"].as_f64().unwrap() > 18.0);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PeriodicTable::default()).unwrap();
    assert_eq!(checked, 2);
}
