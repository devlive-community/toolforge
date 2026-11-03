use super::*;
use serde_json::json;

fn standard_square_placements() -> Vec<Placement> {
    // 经典正方形拼法：七块按分割原位摆回（零平移零旋转）
    vec![
        Placement {
            kind: "large".into(),
            index: 0,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "large".into(),
            index: 1,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "medium".into(),
            index: 2,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "small".into(),
            index: 3,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "small".into(),
            index: 4,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "square".into(),
            index: 5,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
        Placement {
            kind: "parallelogram".into(),
            index: 6,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            flipped: false,
        },
    ]
}

#[test]
fn pieces_have_expected_areas() {
    let all = pieces();
    assert_eq!(all.len(), 7);
    let total: f64 = all.iter().map(|p| p.area).sum();
    assert!(
        (total - 16.0).abs() < 1e-9,
        "classic tangram covers a 4x4 square"
    );
}

#[test]
fn classic_square_arrangement_completes() {
    let out = validate(ValidateArgs {
        outline: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        placements: standard_square_placements(),
    })
    .unwrap();
    assert_eq!(out.status, "complete", "issues: {:?}", out.issues);
}

#[test]
fn overlap_and_outside_are_detected() {
    let mut placements = standard_square_placements();
    // 平移大三角造成越界
    placements[0].x += 6.0;
    let out = validate(ValidateArgs {
        outline: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        placements: placements.clone(),
    })
    .unwrap();
    assert_eq!(out.status, "outside");
    // 复位后制造重叠：小三角挪到与大三角内部重叠处
    placements[0].x = 0.0;
    placements[3].x = 0.5;
    placements[3].y = 1.0;
    let out = validate(ValidateArgs {
        outline: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        placements,
    })
    .unwrap();
    assert!(
        matches!(out.status, "overlap" | "outside"),
        "got {}",
        out.status
    );
}

#[test]
fn area_mismatch_reports() {
    let out = validate(ValidateArgs {
        outline: vec![(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)],
        placements: standard_square_placements(),
    })
    .unwrap();
    assert_eq!(out.status, "area_mismatch");
}

#[test]
fn dispatches_via_plugin() {
    let tool = TangramStudio::default();
    let out = tool.call("pieces", json!({})).unwrap();
    assert_eq!(out.as_array().unwrap().len(), 7);
    let out = tool.call("puzzles", json!({})).unwrap();
    assert!(!out.as_array().unwrap().is_empty());
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TangramStudio::default()).unwrap();
    assert_eq!(checked, 3);
}
