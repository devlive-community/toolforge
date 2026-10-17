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
    for function in ["generate", "validate", "sandbox", "banks"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}

#[test]
fn dispatches_banks_and_rejects_unsupported_selections() {
    let tool = TestCard::default();
    let catalog = tool.call("banks", json!({})).unwrap();
    assert_eq!(catalog["banks"].as_array().unwrap().len(), 18);
    assert_eq!(catalog["banks"][0]["id"], "cmb");
    assert_eq!(catalog["banks"][0]["brand"], "unionpay");
    assert_eq!(catalog["banks"][0]["cardType"], "credit");
    let generated = tool
        .call(
            "generate",
            json!({"brand":"unionpay", "bank":"ccb", "seed":"ccb"}),
        )
        .unwrap();
    assert_eq!(generated["cards"][0]["bank"], "ccb");
    assert_eq!(generated["cards"][0]["bin"].as_str().unwrap().len(), 6);
    assert_eq!(
        tool.call("generate", json!({"brand":"visa", "bank":"cmb"}))
            .unwrap_err()
            .code,
        "card.bank_brand_mismatch"
    );
    assert!(
        tool.call("generate", json!({"brand":"unionpay", "bank":"unknown"}))
            .is_err()
    );
    let omitted = tool
        .call("generate", json!({"brand":"visa", "seed":"old"}))
        .unwrap();
    let explicit_null = tool
        .call(
            "generate",
            json!({"brand":"visa", "bank":null, "seed":"old"}),
        )
        .unwrap();
    assert_eq!(omitted, explicit_null);
    assert!(omitted["cards"][0].get("bank").is_none());
    assert!(omitted["cards"][0].get("bin").is_none());
}
