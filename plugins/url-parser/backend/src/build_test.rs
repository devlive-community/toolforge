use super::*;

fn param(key: &str, value: &str) -> Param {
    Param {
        key: key.into(),
        value: value.into(),
    }
}

#[test]
fn rebuilds_queries() {
    let args = BuildArgs {
        input: "https://example.com/s?old=1#top".into(),
        params: vec![
            param("q", "rust 语言"),
            param("flag", ""),
            param("", ""),
            param("a&b", "c=d"),
        ],
        space_as_plus: false,
        fragment: None,
    };
    assert_eq!(
        build(&args).unwrap(),
        "https://example.com/s?q=rust%20%E8%AF%AD%E8%A8%80&flag&a%26b=c%3Dd#top"
    );
    let plus = BuildArgs {
        space_as_plus: true,
        fragment: Some(String::new()),
        ..args
    };
    assert_eq!(
        build(&plus).unwrap(),
        "https://example.com/s?q=rust+%E8%AF%AD%E8%A8%80&flag&a%26b=c%3Dd"
    );
    let cleared = BuildArgs {
        input: "https://example.com/?a=1".into(),
        params: vec![],
        space_as_plus: false,
        fragment: None,
    };
    assert_eq!(build(&cleared).unwrap(), "https://example.com/");
}

#[test]
fn round_trips_through_parse() {
    let original = "https://example.com/p?name=%E5%BC%A0%E4%B8%89&x=1%2B1&empty=";
    let parsed = parse::parse(original).unwrap();
    let params = parsed
        .params
        .iter()
        .map(|p| param(&p.key, &p.value))
        .collect();
    let rebuilt = build(&BuildArgs {
        input: original.into(),
        params,
        space_as_plus: false,
        fragment: None,
    })
    .unwrap();
    let again = parse::parse(&rebuilt).unwrap();
    let values: Vec<(String, String)> =
        again.params.into_iter().map(|p| (p.key, p.value)).collect();
    assert_eq!(
        values,
        [
            ("name".into(), "张三".into()),
            ("x".into(), "1+1".into()),
            ("empty".into(), String::new())
        ]
    );
}
