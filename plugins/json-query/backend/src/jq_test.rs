use super::*;

const DATA: &str =
    r#"{"users":[{"name":"Ann","age":31,"tags":["admin"]},{"name":"Bob","age":17}],"total":2}"#;

fn texts(code: &str, input: &str, options: Options) -> Vec<String> {
    run(code, input.to_owned(), options)
        .unwrap()
        .results
        .into_iter()
        .map(|o| o.text)
        .collect()
}

#[test]
fn runs_filters() {
    let compact = Options {
        compact: true,
        ..Options::default()
    };
    assert_eq!(
        texts(".users[] | select(.age >= 18) | .name", DATA, compact),
        [r#""Ann""#]
    );
    assert_eq!(texts(".users | map(.age) | add", DATA, compact), ["48"]);
    assert_eq!(
        texts(".users | map({n: .name})", DATA, compact),
        [r#"[{"n":"Ann"},{"n":"Bob"}]"#]
    );
    let raw = Options {
        raw: true,
        ..Options::default()
    };
    assert_eq!(
        texts(r#".users[] | "\(.name) is \(.age)""#, DATA, raw),
        ["Ann is 31", "Bob is 17"]
    );
    assert_eq!(texts(".total", DATA, Options::default()), ["2"]);
    // 与 jq 一致：整数值不带 .0，真正的小数保留
    assert_eq!(
        texts("[.users[].age] | add / length", DATA, compact),
        ["24"]
    );
    assert_eq!(
        texts("[1.5, 4 / 2, {a: 6 / 3}]", "null", compact),
        [r#"[1.5,2,{"a":2}]"#]
    );
}

#[test]
fn pretty_prints_and_sorts_keys() {
    let pretty = texts(
        "{b: 1, a: [2]}",
        "null",
        Options {
            sort_keys: true,
            ..Options::default()
        },
    );
    assert_eq!(pretty, ["{\n  \"a\": [\n    2\n  ],\n  \"b\": 1\n}"]);
}

#[test]
fn runs_once_per_json_lines_value() {
    let lines = "{\"n\":1}\n{\"n\":2}\n{\"n\":3}";
    assert_eq!(
        texts(".n * 10", lines, Options::default()),
        ["10", "20", "30"]
    );
}

#[test]
fn reports_errors_with_positions() {
    let err = run(".users[ | .name", DATA.into(), Options::default()).unwrap_err();
    assert_eq!(err.code, "query.syntax");
    assert_eq!(err.params["line"], 1);
    let undefined = run(".users |\n  nope", DATA.into(), Options::default()).unwrap_err();
    assert_eq!(undefined.code, "query.undefined_filter");
    assert_eq!(undefined.params["name"], "nope");
    assert_eq!(
        (
            undefined.params["line"].as_u64(),
            undefined.params["column"].as_u64()
        ),
        (Some(2), Some(3))
    );
    assert_eq!(
        run("$x", DATA.into(), Options::default()).unwrap_err().code,
        "query.undefined_variable"
    );
    let runtime = run(".users[0] + 1", DATA.into(), Options::default()).unwrap_err();
    assert_eq!(runtime.code, "query.runtime");
    assert_eq!(
        run("  ", DATA.into(), Options::default()).unwrap_err().code,
        "query.empty_query"
    );
}

#[test]
fn caps_endless_output() {
    let out = run("repeat(1)", "null".into(), Options::default()).unwrap();
    assert!(out.truncated);
    assert_eq!(out.results.len(), crate::MAX_RESULTS);
}
