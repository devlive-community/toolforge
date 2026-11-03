use super::*;
use serde_json::json;

#[test]
fn lays_out_trace_cells_and_pads_rows() {
    let out = layout(Args {
        text: "永".into(),
        grid_type: Some("mi".into()),
        per_row: Some(5),
        rows_per_page: Some(4),
        repeat: Some(3),
        mode: Some("trace".into()),
    })
    .unwrap();
    assert_eq!(out.grid_type, "mi");
    assert_eq!(out.pages.len(), 1);
    let row = &out.pages[0].rows[0];
    assert_eq!(row[0].kind, "normal");
    assert_eq!(row[1].kind, "trace");
    assert_eq!(row[2].kind, "trace");
    assert_eq!(row[3].kind, "empty", "repeat 3 fills only 3 of 5 cells");
    assert_eq!(row[4].kind, "empty");
}

#[test]
fn empty_mode_and_spaces() {
    let out = layout(Args {
        text: "好 好".into(),
        grid_type: Some("grid".into()),
        per_row: Some(4),
        rows_per_page: Some(2),
        repeat: Some(1),
        mode: Some("empty".into()),
    })
    .unwrap();
    // 好(空格)好 → 好、空、好 + 1 padding = 4 格一行
    let row = &out.pages[0].rows[0];
    assert_eq!(row[0].kind, "normal");
    assert_eq!(row[1].kind, "empty");
    assert_eq!(row[2].kind, "normal");
    assert_eq!(row[3].kind, "empty");
}

#[test]
fn skips_control_chars_and_reports() {
    let out = layout(Args {
        text: "字\n帖\u{feff}".into(),
        grid_type: None,
        per_row: Some(4),
        rows_per_page: Some(4),
        repeat: Some(1),
        mode: None,
    })
    .unwrap();
    assert_eq!(out.skipped, 2);
    assert_eq!(out.pages[0].rows[0][0].ch, '字');
}

#[test]
fn validates_params_and_empty_text() {
    let err = layout(Args {
        text: String::new(),
        grid_type: None,
        per_row: None,
        rows_per_page: None,
        repeat: None,
        mode: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "copybook.empty_text");
    let err = layout(Args {
        text: "字".into(),
        grid_type: Some("wrong".into()),
        per_row: None,
        rows_per_page: None,
        repeat: None,
        mode: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "copybook.bad_grid");
    let err = layout(Args {
        text: "字".into(),
        grid_type: None,
        per_row: Some(2),
        rows_per_page: None,
        repeat: None,
        mode: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "copybook.bad_per_row");
    let err = layout(Args {
        text: "字".repeat(5000),
        grid_type: None,
        per_row: Some(16),
        rows_per_page: Some(20),
        repeat: Some(10),
        mode: Some("trace".into()),
    })
    .unwrap_err();
    assert_eq!(err.code, "copybook.too_many_pages");
}

#[test]
fn dispatches_via_plugin() {
    let out = HanziCopybook::default()
        .call(
            "layout",
            json!({"text": "一二三", "perRow": 4, "repeat": 2, "rowsPerPage": 2}),
        )
        .unwrap();
    assert_eq!(out["pages"].as_array().unwrap().len(), 1);
    assert_eq!(
        HanziCopybook::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&HanziCopybook::default()).unwrap();
    assert_eq!(checked, 1);
}
