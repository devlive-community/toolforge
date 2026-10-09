use super::*;

const DATA: &str =
    r#"{"store":{"book":[{"title":"A","price":8},{"title":"B","price":12}],"bike":{"price":99}}}"#;

#[test]
fn queries_with_locations() {
    let out = run(
        "$..price",
        DATA,
        &Options {
            compact: true,
            ..Options::default()
        },
    )
    .unwrap();
    let pairs: Vec<(String, String)> = out
        .results
        .into_iter()
        .map(|o| (o.path.unwrap(), o.text))
        .collect();
    assert_eq!(pairs.len(), 3);
    assert!(pairs.contains(&("$['store']['book'][1]['price']".into(), "12".into())));
    let cheap = run(
        "$.store.book[?@.price < 10].title",
        DATA,
        &Options {
            raw: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(cheap.results[0].text, "A");
}

#[test]
fn sorts_keys_recursively() {
    let out = run(
        "$",
        r#"{"b":{"d":1,"c":2},"a":0}"#,
        &Options {
            compact: true,
            sort_keys: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(out.results[0].text, r#"{"a":0,"b":{"c":2,"d":1}}"#);
}

#[test]
fn reports_parse_errors() {
    let err = run("$.store[", DATA, &Options::default()).unwrap_err();
    assert_eq!(err.code, "query.jsonpath");
    assert!(err.params["column"].as_u64().unwrap() >= 1);
}
