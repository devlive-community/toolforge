use super::*;

#[test]
fn locates_values_by_pointer() {
    let text = "{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n    {\"type\": \"Feature\", \"geometry\": {\"type\": \"Point\", \"coordinates\": [1, 2]}}\n  ]\n}";
    let spans = scan(text).unwrap();
    // 根对象与成员都能定位到行列
    assert_eq!(locate(&spans, ""), Some((1, 1)));
    let (line, column) = locate(&spans, "/features/0/geometry/coordinates/1").unwrap();
    // 第二个坐标在样例第 4 行第 74 列
    assert_eq!(line, 4);
    assert_eq!(column, 74);
    // 不存在的 pointer 返回 None
    assert_eq!(locate(&spans, "/nope"), None);
}

#[test]
fn escapes_special_characters_in_keys() {
    let text = r#"{"a/b": {"c~d": 1}}"#;
    let spans = scan(text).unwrap();
    // RFC 6901：/ 转义为 ~1，~ 转义为 ~0
    assert!(locate(&spans, "/a~1b/c~0d").is_some());
}

#[test]
fn tracks_line_and_column_across_multiline_strings() {
    let text = "{\n  \"type\": \"Point\",\n  \"coordinates\": [\n    1.5,\n    2.5\n  ]\n}";
    let spans = scan(text).unwrap();
    assert_eq!(locate(&spans, "/coordinates/0"), Some((4, 5)));
    assert_eq!(locate(&spans, "/coordinates/1"), Some((5, 5)));
}

#[test]
fn handles_nested_arrays_and_scalars() {
    let text = r#"{"t": true, "n": null, "x": [1, {"deep": [[3]]}], "s": "str", "f": 1.5}"#;
    let spans = scan(text).unwrap();
    assert!(locate(&spans, "/t").is_some());
    assert!(locate(&spans, "/n").is_some());
    assert!(locate(&spans, "/x/1/deep/0/0").is_some());
    assert!(locate(&spans, "/s").is_some());
    assert!(locate(&spans, "/f").is_some());
}

#[test]
fn depth_limit_returns_error() {
    let mut text = String::new();
    for _ in 0..(MAX_DEPTH + 4) {
        text.push('[');
    }
    for _ in 0..(MAX_DEPTH + 4) {
        text.push(']');
    }
    // serde_json 能解析，但扫描器按深度上限拒绝
    assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
    let err = scan(&text).unwrap_err();
    assert_eq!(err.kind, "depth");
}
