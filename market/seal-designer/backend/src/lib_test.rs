use super::*;
use serde_json::json;

#[test]
fn four_char_layout_and_reading_order() {
    let out = render(Args {
        text: "平安喜乐".into(),
        shape: Some("square".into()),
        style: Some("zhuwen".into()),
        size_mm: Some(40.0),
        border_mm: Some(2.0),
        mirror: false,
        transparent: false,
    })
    .unwrap();
    assert_eq!(
        out.order,
        vec!["平", "安", "喜", "乐"],
        "reading order matches the input text"
    );
    assert!(out.svg.contains(">乐</text>"));
    assert!(out.svg.contains("fill='#b91c1c'"), "zhuwen is red text");
    assert!(out.svg.contains("fill='none'"), "zhuwen face is not filled");
}

#[test]
fn baiwen_inverts_colors_and_mirror_transforms() {
    let out = render(Args {
        text: "墨缘".into(),
        shape: Some("round".into()),
        style: Some("baiwen".into()),
        size_mm: None,
        border_mm: None,
        mirror: true,
        transparent: true,
    })
    .unwrap();
    assert!(out.svg.contains("scale(-1,1)"), "mirror preview");
    assert!(out.svg.contains("fill='#ffffff'"), "baiwen text is white");
    assert!(out.svg.contains("circle"), "round seal");
}

#[test]
fn validates_counts_shapes_and_styles() {
    let err = render(Args {
        text: "一".into(),
        shape: None,
        style: None,
        size_mm: None,
        border_mm: None,
        mirror: false,
        transparent: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "seal.bad_char_count");
    let err = render(Args {
        text: "一二三四五".into(),
        shape: None,
        style: None,
        size_mm: None,
        border_mm: None,
        mirror: false,
        transparent: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "seal.bad_char_count");
    let err = render(Args {
        text: "一二".into(),
        shape: Some("hex".into()),
        style: None,
        size_mm: None,
        border_mm: None,
        mirror: false,
        transparent: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "seal.bad_shape");
    let err = render(Args {
        text: "一二".into(),
        shape: None,
        style: Some("green".into()),
        size_mm: None,
        border_mm: None,
        mirror: false,
        transparent: false,
    })
    .unwrap_err();
    assert_eq!(err.code, "seal.bad_style");
}

#[test]
fn dispatches_via_plugin() {
    let out = SealDesigner::default()
        .call("render", json!({"text": "王二"}))
        .unwrap();
    assert_eq!(out["order"].as_array().unwrap().len(), 2);
    assert_eq!(
        SealDesigner::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&SealDesigner::default()).unwrap();
    assert_eq!(checked, 1);
}
