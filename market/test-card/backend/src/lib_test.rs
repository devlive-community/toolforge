use super::*;

#[test]
fn dispatches_functions() {
    let tool = TestCard::default();
    let out = tool
        .call(
            "generate",
            json!({"brand": "amex", "count": 2, "seed": "x"}),
        )
        .unwrap();
    assert_eq!(out["cards"].as_array().unwrap().len(), 2);
    let sandbox = tool.call("sandbox", json!({})).unwrap();
    assert!(sandbox["cases"].as_array().unwrap().len() > 30);
    for function in ["generate", "validate", "sandbox"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
