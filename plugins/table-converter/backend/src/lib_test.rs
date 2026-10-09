use serde_json::json;

use super::*;

#[test]
fn converts_excel_paste_to_markdown() {
    let out = TableConverter::default()
        .call(
            "convert",
            json!({ "input": "城市\t人口\n北京\t2189\n上海\t2487\n", "to": "markdown" }),
        )
        .unwrap();
    assert_eq!(out["detected"], "tsv");
    assert_eq!(out["header"], true);
    assert_eq!(
        (out["columns"].as_u64(), out["rows"].as_u64()),
        (Some(2), Some(2))
    );
    assert_eq!(
        out["output"],
        "| 城市 | 人口 |\n| ---- | ---- |\n| 北京 | 2189 |\n| 上海 | 2487 |"
    );
}

#[test]
fn applies_options() {
    let tool = TableConverter::default();
    let out = tool
        .call(
            "convert",
            json!({ "input": "a,b\n1,2", "to": "csv", "header": false, "transpose": true }),
        )
        .unwrap();
    assert_eq!(out["output"], "a,1\nb,2");
    let aligned = tool
        .call(
            "convert",
            json!({ "input": "| x |\n|---|\n| 1 |", "to": "markdown", "aligns": ["center"] }),
        )
        .unwrap();
    assert_eq!(
        aligned["output"].as_str().unwrap().lines().nth(1),
        Some("| :-: |")
    );
}

#[test]
fn detects_tables_on_the_clipboard() {
    let tool = TableConverter::default();
    assert_eq!(tool.detect("a\tb\n1\t2").unwrap().label, "tsv");
    assert_eq!(
        tool.detect("| a |\n|---|\n| 1 |").unwrap().label,
        "markdown"
    );
    assert!(tool.detect("just text").is_none());
}
