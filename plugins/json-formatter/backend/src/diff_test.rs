use super::*;

fn diff(left: &str, right: &str) -> Report {
    run(Args {
        left: left.into(),
        right: right.into(),
        mode: Mode::Json,
    })
    .unwrap()
}

#[test]
fn identical_documents_have_no_changes() {
    let report = diff(r#"{"a":[1,{"b":2}]}"#, r#"{ "a": [1, {"b": 2}] }"#);
    assert!(report.items.is_empty());
}

#[test]
fn detects_added_removed_and_changed() {
    let report = diff(
        r#"{"a":1,"b":2,"list":[1,2]}"#,
        r#"{"a":1,"b":3,"c":4,"list":[1]}"#,
    );
    assert_eq!((report.added, report.removed, report.changed), (1, 1, 1));
    let paths: Vec<&str> = report.items.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, vec!["$.b", "$.list[1]", "$.c"]);
}

#[test]
fn quotes_non_identifier_keys() {
    let report = diff(r#"{"a b":1}"#, r#"{"a b":2}"#);
    assert_eq!(report.items[0].path, "$[\"a b\"]");
}

#[test]
fn parse_errors_name_the_side() {
    let err = run(Args {
        left: "{}".into(),
        right: "{".into(),
        mode: Mode::Json,
    })
    .unwrap_err();
    assert_eq!(err.params["side"], "right");
}
