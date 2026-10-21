use super::*;

fn model(text: &str) -> PluginResult<Compiled> {
    normalize(parse_json(text)?)
}

#[test]
fn normalizes_orders_and_decimal_values_without_float_rounding() {
    let a = model(r#"{"factors":[{"name":" B ","values":["1",1.0,true,null]},{"name":"A","values":[9007199254740993,2]}]}"#).unwrap();
    let b = model(r#"{"factors":[{"name":"A","values":[2,9007199254740993]},{"name":"B","values":[null,true,1e0,"1"]}]}"#).unwrap();
    assert_eq!(a.model_text(), b.model_text());
    assert!(a.model_text().contains("9007199254740993"));
    assert_eq!(a.combinations, 8);
    assert_eq!(a.model.factors[1].values.len(), 4);
}

#[test]
fn normalizes_numeric_equivalents_and_preserves_scalar_types() {
    for (a, b) in [
        ("1", "1.000"),
        ("1e2", "100"),
        ("-0.0", "0"),
        ("0.00100", "1e-3"),
        ("-10.00", "-1e1"),
    ] {
        assert_eq!(
            scalar(&parse_json::<Value>(a).unwrap()).unwrap(),
            scalar(&parse_json::<Value>(b).unwrap()).unwrap()
        );
    }
    let keys = [json!("1"), json!(1), json!(true), Value::Null].map(|v| scalar(&v).unwrap().0);
    assert_eq!(keys.into_iter().collect::<BTreeSet<_>>().len(), 4);
}

#[test]
fn rejects_duplicates_empty_factors_and_invalid_names() {
    for (text, code) in [
        (r#"{"factors":[]}"#, "factors"),
        (
            r#"{"factors":[{"name":"A","values":[1]},{"name":" A ","values":[2]}]}"#,
            "duplicate_factor",
        ),
        (
            r#"{"factors":[{"name":"A","values":[]},{"name":"B","values":[2]}]}"#,
            "values",
        ),
        (
            r#"{"factors":[{"name":"A","values":[1,1.0]},{"name":"B","values":[2]}]}"#,
            "duplicate_value",
        ),
        (
            r#"{"factors":[{"name":"","values":[1]},{"name":"B","values":[2]}]}"#,
            "factor_name",
        ),
        (
            r#"{"factors":[{"name":"A","values":[{}]},{"name":"B","values":[2]}]}"#,
            "scalar",
        ),
    ] {
        assert_eq!(model(text).err().unwrap().code, format!("pairwise.{code}"));
    }
}

#[test]
fn rejects_constraint_references_and_deduplicates_equivalent_tuples() {
    let base =
        r#"{"factors":[{"name":"A","values":[1]},{"name":"B","values":["1"]}],"forbidden":[]}"#;
    for tuple in [
        json!([]),
        json!([{"factor":"X","value":1}]),
        json!([{"factor":"A","value":"1"}]),
        json!([{"factor":"A","value":1},{"factor":"A","value":1}]),
    ] {
        let mut raw: Value = parse_json(base).unwrap();
        raw["forbidden"] = json!([tuple]);
        let m: Model = serde_json::from_value(raw).unwrap();
        assert_eq!(
            normalize(m).err().unwrap().code,
            "pairwise.constraint_reference"
        );
    }
    let mut m: Model = parse_json(base).unwrap();
    m.forbidden = vec![
        vec![
            Term {
                factor: "B".into(),
                value: json!("1"),
            },
            Term {
                factor: "A".into(),
                value: json!(1),
            },
        ],
        vec![
            Term {
                factor: "A".into(),
                value: json!(1.0),
            },
            Term {
                factor: "B".into(),
                value: json!("1"),
            },
        ],
    ];
    assert_eq!(normalize(m).unwrap().constraints.len(), 1);
}

#[test]
fn enforces_model_and_input_budgets() {
    let mut m = Model {
        factors: (0..12)
            .map(|n| Factor {
                name: format!("F{n}"),
                values: (0..12).map(|n| json!(n)).collect(),
            })
            .collect(),
        forbidden: vec![],
    };
    assert_eq!(
        normalize(m.clone()).err().unwrap().code,
        "pairwise.combination_limit"
    );
    m.factors.truncate(2);
    m.factors[0].values = vec![json!("x".repeat(241))];
    assert_eq!(normalize(m).err().unwrap().code, "pairwise.scalar");
    assert_eq!(
        parse_json::<Value>(&" ".repeat(MAX_INPUT + 1))
            .err()
            .unwrap()
            .code,
        "pairwise.input_limit"
    );
    assert!(scalar(&parse_json::<Value>("1e1001").unwrap()).is_err());
}

#[test]
fn draft_round_trips_through_the_rust_parser() {
    let raw = json!({"factors":[{"name":"OS","valuesText":"[\"Linux\",\"Windows\"]"},{"name":"Role","valuesText":"[true,false]"}],"forbiddenText":"[]"});
    let compiled = normalize(from_draft(serde_json::from_value(raw).unwrap()).unwrap()).unwrap();
    let value = compiled.draft();
    let restored = Draft {
        factors: serde_json::from_value(value["factors"].clone()).unwrap(),
        forbidden_text: value["forbiddenText"].as_str().unwrap().into(),
    };
    assert_eq!(
        normalize(from_draft(restored).unwrap())
            .unwrap()
            .model_text(),
        compiled.model_text()
    );
}

#[test]
fn numeric_normalization_is_idempotent_at_the_size_and_exponent_limits() {
    for text in [
        format!("{}0", "1".repeat(79)),
        format!("0.{}", "1".repeat(78)),
        format!("-0.{}", "1".repeat(77)),
        "1e1000".to_owned(),
        "1e-1000".to_owned(),
    ] {
        let (_, once) = scalar(&parse_json::<Value>(&text).unwrap()).unwrap();
        assert!(once.to_string().len() <= 80);
        let (_, twice) = scalar(&once).unwrap();
        assert_eq!(once, twice);
    }
    for text in ["10e1000", "0.1e-1000"] {
        assert_eq!(
            scalar(&parse_json::<Value>(text).unwrap())
                .err()
                .unwrap()
                .code,
            "pairwise.scalar"
        );
    }
}
