use super::*;
use serde_json::json;

#[test]
fn solves_circle_and_cuboid() {
    let out = solve(ShapeArgs {
        shape: "circle".into(),
        r: Some(3.0),
        ..Default::default()
    })
    .unwrap();
    assert!((out.rows[0].value - 18.849_555_9).abs() < 1e-6);
    assert!((out.rows[1].value - 28.274_333_9).abs() < 1e-6);
    assert_eq!(out.kind, "plane");

    let out = solve(ShapeArgs {
        shape: "cuboid".into(),
        w: Some(2.0),
        l: Some(3.0),
        h: Some(4.0),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(out.rows[0].value, 52.0);
    assert_eq!(out.rows[1].value, 24.0);
    assert_eq!(out.kind, "solid");
}

#[test]
fn heron_triangle_rejects_invalid_sides() {
    let out = solve(ShapeArgs {
        shape: "triangle".into(),
        a: Some(3.0),
        b: Some(4.0),
        c: Some(5.0),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(out.rows[0].value, 12.0);
    assert!((out.rows[1].value - 6.0).abs() < 1e-12);

    let err = solve(ShapeArgs {
        shape: "triangle".into(),
        a: Some(1.0),
        b: Some(2.0),
        c: Some(3.0),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code, "geometry.bad_triangle");
}

#[test]
fn missing_and_nonpositive_params_are_rejected() {
    let err = solve(ShapeArgs {
        shape: "circle".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code, "geometry.missing_param");
    let err = solve(ShapeArgs {
        shape: "circle".into(),
        r: Some(0.0),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code, "geometry.positive_required");
    let err = solve(ShapeArgs {
        shape: "hexagon".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code, "geometry.unknown_shape");
}

#[test]
fn paint_and_tile_estimates() {
    // 4×5m、高 2.8m 的房间，开口 4m²，覆盖率 10m²/L，两遍
    let out = materials(MaterialArgs {
        kind: "paint".into(),
        length: 4.0,
        width: 5.0,
        height: Some(2.8),
        openings: Some(4.0),
        coverage: Some(10.0),
        coats: Some(2.0),
        tile_length: None,
        tile_width: None,
        waste: None,
    })
    .unwrap();
    let wall = 2.0 * (4.0 + 5.0) * 2.8 - 4.0;
    assert_eq!(out.rows[0].value, wall);
    assert!((out.rows[1].value - wall * 2.0 / 10.0).abs() < 1e-9);
    assert_eq!(out.buy, (wall * 2.0 / 10.0).ceil());
    assert_eq!(out.unit, "liters");

    // 5×4m 地面铺 0.8×0.8m 砖，损耗 10%
    let out = materials(MaterialArgs {
        kind: "tiles".into(),
        length: 5.0,
        width: 4.0,
        height: None,
        openings: None,
        coverage: None,
        coats: None,
        tile_length: Some(0.8),
        tile_width: Some(0.8),
        waste: Some(10.0),
    })
    .unwrap();
    let pieces = ((5.0f64 * 4.0 * 1.1) / 0.64).ceil();
    assert_eq!(out.buy, pieces);
    assert_eq!(out.unit, "pieces");
}

#[test]
fn dispatches_via_plugin() {
    let tool = GeometryCalculator::default();
    let out = tool
        .call("solve", json!({"shape": "rectangle", "w": 2, "l": 3}))
        .unwrap();
    assert_eq!(out["rows"][0]["value"], 10.0);
    let out = tool
        .call(
            "materials",
            json!({"kind": "tiles", "length": 5, "width": 4, "tileLength": 0.8, "tileWidth": 0.8}),
        )
        .unwrap();
    assert_eq!(out["unit"], "pieces");
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&GeometryCalculator::default())
            .unwrap();
    assert_eq!(checked, 2);
}
