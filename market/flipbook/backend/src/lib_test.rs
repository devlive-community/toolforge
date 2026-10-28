use super::*;

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    // 宿主按 manifest.functions 放行调用，漏声明会在界面报「插件不支持该操作」
    let plugin = Flipbook::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn draw_then_duplicate_then_draw_keeps_the_original_frame() {
    let plugin = Flipbook::default();
    let project = plugin
        .call(
            "create",
            json!({ "id": "w1", "name": "Cat", "width": 240, "height": 180 }),
        )
        .unwrap();
    let stroke = |frame: &str, x: f32| {
        plugin
            .call(
                "stroke",
                json!({ "width": 240, "height": 180, "frame": frame,
                        "stroke": { "tool": "brush", "color": "#000000", "size": 4, "points": [[x, 10.0]] } }),
            )
            .unwrap()["frame"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let first = stroke("", 10.0);
    let edit = plugin
        .call(
            "edit",
            json!({ "project": project, "op": { "kind": "duplicate", "index": 0 } }),
        )
        .unwrap();
    assert_eq!(edit["created"]["source"], json!("f1"));
    // 复制出的帧在原内容基础上再画一笔，原帧文本不变
    let second = stroke(&first, 100.0);
    assert_ne!(first, second);
    let original = frame::decode(&first, 240, 180).unwrap();
    assert_eq!(original.get_pixel(100, 10)[3], 0);
}

#[test]
fn limits_describe_sizes_and_delay_range() {
    let out = Flipbook::default().call("limits", json!({})).unwrap();
    assert_eq!(out["minDelay"], json!(20));
    assert_eq!(out["maxDelay"], json!(2000));
    assert!(out["sizes"].as_array().unwrap().len() >= 4);
}

#[test]
fn import_of_missing_path_is_rejected() {
    let err = Flipbook::default()
        .call("import", json!({ "width": 240, "height": 180, "path": "" }))
        .unwrap_err();
    assert_eq!(err.code, "flipbook.bad_path");
}
