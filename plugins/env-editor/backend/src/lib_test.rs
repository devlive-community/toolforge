use super::*;

#[test]
fn exposes_the_document_functions() {
    let plugin = EnvEditor::default();
    assert_eq!(plugin.manifest().id, "org.devlive.toolforge.env-editor");
    assert!(plugin.call("open", serde_json::json!({})).is_err());
    assert!(plugin.call("nope", serde_json::json!({})).is_err());
}

#[test]
fn opens_saves_and_exports_examples() {
    let dir = std::env::temp_dir().join(format!("tfp-env-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(".env");
    std::fs::write(&path, "# config\nHOST=localhost\n").unwrap();

    let plugin = EnvEditor::default();
    let opened: OpenResult = serde_json::from_value(
        plugin
            .call(
                "open",
                serde_json::json!({ "path": path.to_string_lossy() }),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(opened.lines.len(), 2);
    assert_eq!(
        opened.lines[0],
        document::Line::Comment {
            text: "# config".to_owned()
        }
    );

    // 编辑值后保存
    let mut lines = opened.lines;
    lines[1] = serde_json::from_value(serde_json::json!({
        "kind": "entry", "export": false, "key": "HOST",
        "value": "example.org", "quote": "none",
        "inlineComment": null, "multiline": false
    }))
    .unwrap();
    let saved: SaveResult = serde_json::from_value(
        plugin
            .call(
                "save",
                serde_json::json!({ "path": path.to_string_lossy(), "lines": lines }),
            )
            .unwrap(),
    )
    .unwrap();
    assert!(saved.bytes > 0);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# config\nHOST=example.org\n"
    );

    // 重复 key 被拒绝
    let dup: Vec<document::Line> = vec![
        serde_json::from_value(serde_json::json!({ "kind": "entry", "export": false, "key": "A", "value": "1", "quote": "none", "inlineComment": null, "multiline": false })).unwrap(),
        serde_json::from_value(serde_json::json!({ "kind": "entry", "export": false, "key": "A", "value": "2", "quote": "none", "inlineComment": null, "multiline": false })).unwrap(),
    ];
    let err = plugin
        .call(
            "save",
            serde_json::json!({ "path": path.to_string_lossy(), "lines": dup }),
        )
        .unwrap_err();
    assert_eq!(err.code, "env.duplicate_key");

    // 导出 example
    let example: ExampleResult = serde_json::from_value(
        plugin
            .call(
                "export_example",
                serde_json::json!({
                    "path": path.to_string_lossy(),
                    "lines": [serde_json::json!({ "kind": "entry", "export": false, "key": "HOST", "value": "example.org", "quote": "none", "inlineComment": null, "multiline": false })]
                }),
            )
            .unwrap(),
    )
    .unwrap();
    assert!(example.path.ends_with(".env.example"));
    assert_eq!(std::fs::read_to_string(&example.path).unwrap(), "HOST=\n");

    // 已存在的 example 被拒绝
    let err = plugin
        .call(
            "export_example",
            serde_json::json!({
                "path": path.to_string_lossy(),
                "lines": [serde_json::json!({ "kind": "entry", "export": false, "key": "HOST", "value": "x", "quote": "none", "inlineComment": null, "multiline": false })]
            }),
        )
        .unwrap_err();
    assert_eq!(err.code, "env.example_exists");
}
