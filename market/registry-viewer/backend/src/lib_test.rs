use super::*;

#[test]
fn rejects_unknown_functions() {
    let error = RegistryViewer::default()
        .call("probe", serde_json::json!({}))
        .unwrap_err();
    assert_eq!(error.code, "plugin.function_not_found");
}

/// 检查值数量统计（parse 返回结构）
#[test]
fn parse_counts_values() {
    let text = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\A]
"x"="1"
"y"="2"

[HKEY_CURRENT_USER\Software\B]
@="d"
"#;
    let doc = doc::parse(text).unwrap();
    let value_count = doc.keys.iter().map(|k| k.values.len()).sum::<usize>();
    assert_eq!(value_count, 3);
}
