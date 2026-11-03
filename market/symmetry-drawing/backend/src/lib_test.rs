use super::*;
use serde_json::json;

fn stroke(points: &[f64], mode: &str, folds: usize) -> Stroke {
    Stroke {
        points: points.to_vec(),
        color: Some("#ff0000".into()),
        width: Some(2.0),
        opacity: Some(1.0),
        mode: Some(mode.into()),
        folds: Some(folds),
        center: Some((100.0, 100.0)),
    }
}

fn export(strokes: Vec<Stroke>) -> String {
    export_svg(&ExportArgs {
        strokes,
        width: 200.0,
        height: 200.0,
        background: Some("#ffffff".into()),
    })
    .unwrap()
}

#[test]
fn mirror_modes_double_or_quadruple_paths() {
    let one = export(vec![stroke(&[100.0, 20.0, 120.0, 40.0], "none", 1)]);
    assert_eq!(one.matches("<path").count(), 1);
    let two = export(vec![stroke(&[100.0, 20.0, 120.0, 40.0], "mirror_x", 1)]);
    assert_eq!(two.matches("<path").count(), 2);
    let four = export(vec![stroke(&[100.0, 20.0, 120.0, 40.0], "mirror_xy", 1)]);
    assert_eq!(four.matches("<path").count(), 4);
}

#[test]
fn rotate_produces_one_path_per_fold() {
    let six = export(vec![stroke(&[100.0, 20.0, 110.0, 30.0], "rotate", 6)]);
    assert_eq!(six.matches("<path").count(), 6);
    // 旋转后的点落在同一半径上
    assert!(six.contains("M100.00 20.00"));
}

#[test]
fn transparent_export_has_no_background_rect() {
    let svg = export_svg(&ExportArgs {
        strokes: vec![stroke(&[10.0, 10.0, 20.0, 20.0], "none", 1)],
        width: 100.0,
        height: 100.0,
        background: Some("transparent".into()),
    })
    .unwrap();
    assert!(!svg.contains("<rect"));
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn validates_strokes_and_params() {
    let err = export_svg(&ExportArgs {
        strokes: vec![stroke(&[10.0], "none", 1)],
        width: 100.0,
        height: 100.0,
        background: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "symmetry.bad_stroke");
    let err = export_svg(&ExportArgs {
        strokes: vec![stroke(&[10.0, 10.0, 20.0, 20.0], "rotate", 20)],
        width: 100.0,
        height: 100.0,
        background: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "symmetry.bad_folds");
    let err = export_svg(&ExportArgs {
        strokes: vec![stroke(&[10.0, 10.0, 20.0, 20.0], "none", 1)],
        width: 100.0,
        height: 100.0,
        background: Some("javascript:alert(1)".into()),
    })
    .unwrap_err();
    assert_eq!(err.code, "symmetry.bad_color");
}

#[test]
fn dispatches_via_plugin() {
    let out = SymmetryDrawing::default()
        .call(
            "export_svg",
            json!({
                "strokes": [{"points": [100.0, 10.0, 110.0, 20.0], "mode": "rotate", "folds": 4, "center": [100.0, 100.0]}],
                "width": 200.0,
                "height": 200.0
            }),
        )
        .unwrap();
    assert!(out["svg"].as_str().unwrap().contains("<path"));
    assert_eq!(
        SymmetryDrawing::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&SymmetryDrawing::default()).unwrap();
    assert_eq!(checked, 1);
}
