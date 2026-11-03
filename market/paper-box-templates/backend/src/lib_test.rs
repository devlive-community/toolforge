use super::*;
use serde_json::json;

#[test]
fn box_sheet_dimensions_and_fit() {
    let out = box_template(BoxArgs {
        length: 60.0,
        width: 40.0,
        height: 30.0,
        glue: Some(12.0),
        tab: Some(10.0),
        paper_long: None,
        paper_short: None,
    })
    .unwrap();
    // 12+30+60+30+60+12 = 204 × (12+40+10+12=74)
    assert!((out.sheet_long - 204.0).abs() < 0.01);
    assert!((out.sheet_short - 74.0).abs() < 0.01);
    assert!(out.fits);
    assert!(
        out.svg.contains("stroke-dasharray"),
        "fold lines are dashed"
    );
    assert!(out.svg.contains("hatch"), "glue areas hatched");
}

#[test]
fn box_rejects_bad_sizes() {
    let err = box_template(BoxArgs {
        length: 0.0,
        width: 40.0,
        height: 30.0,
        glue: None,
        tab: None,
        paper_long: None,
        paper_short: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "template.positive_required");
    let err = box_template(BoxArgs {
        length: 60.0,
        width: 40.0,
        height: 30.0,
        glue: Some(2.0),
        tab: None,
        paper_long: None,
        paper_short: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "template.bad_tab_glue");
    let err = box_template(BoxArgs {
        length: 600.0,
        width: 40.0,
        height: 30.0,
        glue: None,
        tab: None,
        paper_long: None,
        paper_short: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "template.too_large");
    let out = box_template(BoxArgs {
        length: 200.0,
        width: 150.0,
        height: 80.0,
        glue: None,
        tab: None,
        paper_long: None,
        paper_short: None,
    })
    .unwrap();
    assert!(!out.fits, "oversized box does not fit A4");
}

#[test]
fn envelope_covers_card_with_clearance() {
    let out = envelope_template(EnvelopeArgs {
        card_length: 120.0,
        card_width: 80.0,
        clearance: Some(3.0),
    })
    .unwrap();
    assert!(out.fits);
    assert!(out.svg.contains("rect"));
    let bigger = envelope_template(EnvelopeArgs {
        card_length: 400.0,
        card_width: 300.0,
        clearance: Some(10.0),
    })
    .unwrap();
    assert!(!bigger.fits);
    let err = envelope_template(EnvelopeArgs {
        card_length: -1.0,
        card_width: 80.0,
        clearance: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "template.positive_required");
}

#[test]
fn dispatches_via_plugin() {
    let out = PaperBoxTemplates::default()
        .call("box", json!({"length": 50, "width": 40, "height": 30}))
        .unwrap();
    assert_eq!(out["fits"], true);
    let out = PaperBoxTemplates::default()
        .call("envelope", json!({"cardLength": 100, "cardWidth": 70}))
        .unwrap();
    assert!(out["svg"].as_str().unwrap().contains("svg"));
    assert_eq!(
        PaperBoxTemplates::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PaperBoxTemplates::default()).unwrap();
    assert_eq!(checked, 2);
}
