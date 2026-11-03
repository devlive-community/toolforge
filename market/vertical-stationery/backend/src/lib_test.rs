use super::*;
use serde_json::json;

#[test]
fn lays_columns_right_to_left_with_title_and_signature() {
    let out = layout(Args {
        text: "明月松间照清泉石上流。".into(),
        title: Some("山居秋暝".into()),
        signature: Some("王维".into()),
        paper_width: Some(150.0),
        paper_height: Some(220.0),
        font_mm: Some(7.0),
        per_column: Some(6),
    })
    .unwrap();
    assert_eq!(out.columns, 1 + 2 + 1, "title + 2 body columns + signature");
    assert_eq!(out.pages, 1);
    // 标点是竖排形态
    assert!(out.svg.contains('。') || out.svg.contains("、"));
}

#[test]
fn punctuation_maps_to_vertical_forms() {
    assert_eq!(vertical_char('，'), "、");
    assert_eq!(vertical_char('（'), "︵");
    assert_eq!(vertical_char('a'), "a");
}

#[test]
fn validates_inputs() {
    let err = layout(Args {
        text: String::new(),
        title: None,
        signature: None,
        paper_width: None,
        paper_height: None,
        font_mm: None,
        per_column: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "stationery.empty_text");
    let err = layout(Args {
        text: "字".into(),
        title: None,
        signature: None,
        paper_width: Some(50.0),
        paper_height: None,
        font_mm: None,
        per_column: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "stationery.bad_paper");
    let err = layout(Args {
        text: "字".into(),
        title: None,
        signature: None,
        paper_width: None,
        paper_height: None,
        font_mm: Some(25.0),
        per_column: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "stationery.bad_font");
    let err = layout(Args {
        text: "字".into(),
        title: None,
        signature: None,
        paper_width: None,
        paper_height: None,
        font_mm: None,
        per_column: Some(40),
    })
    .unwrap_err();
    assert_eq!(err.code, "stationery.bad_per_column");
}

#[test]
fn dispatches_via_plugin() {
    let out = VerticalStationery::default()
        .call(
            "layout",
            json!({"text": "春眠不觉晓", "title": "春晓", "signature": "孟浩然"}),
        )
        .unwrap();
    assert!(out["svg"].as_str().unwrap().contains("svg"));
    assert_eq!(
        VerticalStationery::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&VerticalStationery::default())
            .unwrap();
    assert_eq!(checked, 1);
}
