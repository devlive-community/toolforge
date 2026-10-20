use super::*;
use crate::spans::scan;
use serde_json::json;

fn check(value: Value) -> Report {
    let text = serde_json::to_string(&value).unwrap();
    let spans = scan(&text).unwrap();
    run(&value, &spans)
}

fn codes(report: &Report) -> Vec<&str> {
    report.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn has(report: &Report, code: &str) -> bool {
    report.diagnostics.iter().any(|d| d.code == code)
}

#[test]
fn valid_feature_collection_passes() {
    let report = check(json!({
        "type": "FeatureCollection",
        "features": [
            {"type": "Feature", "properties": {"name": "a"}, "geometry": {"type": "Point", "coordinates": [102.0, 0.5]}},
            {"type": "Feature", "properties": null, "geometry": null}
        ],
        "bbox": [100.0, 0.0, 103.0, 1.0]
    }));
    assert_eq!(report.overall, "passed");
    assert!(codes(&report).is_empty());
    assert_eq!(report.stats.features, 2);
    assert_eq!(report.stats.positions, 1);
    assert_eq!(report.stats.types.get("Point"), Some(&1));
}

#[test]
fn all_seven_geometry_types_are_recognized() {
    let report = check(json!({
        "type": "GeometryCollection",
        "geometries": [
            {"type": "Point", "coordinates": [1.0, 2.0]},
            {"type": "MultiPoint", "coordinates": [[1.0, 2.0], [3.0, 4.0]]},
            {"type": "LineString", "coordinates": [[1.0, 2.0], [3.0, 4.0]]},
            {"type": "MultiLineString", "coordinates": [[[1.0, 2.0], [3.0, 4.0]]]},
            {"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]},
            {"type": "MultiPolygon", "coordinates": [[[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]]},
            {"type": "GeometryCollection", "geometries": [{"type": "MultiPoint", "coordinates": [[1.0, 2.0]]}]}
        ]
    }));
    assert_eq!(report.overall, "passed");
    // 嵌套 GeometryCollection 里的类型同样计数
    assert_eq!(report.stats.types.get("Point"), Some(&1), "Point");
    assert_eq!(report.stats.types.get("MultiPoint"), Some(&2), "MultiPoint");
    assert_eq!(
        report.stats.types.get("GeometryCollection"),
        Some(&2),
        "GeometryCollection"
    );
    for kind in ["LineString", "MultiLineString", "Polygon", "MultiPolygon"] {
        assert_eq!(report.stats.types.get(kind), Some(&1), "{kind}");
    }
}

#[test]
fn missing_or_wrong_type_members_fail() {
    let report = check(json!({"type": "Point"}));
    assert!(has(&report, "geojson.coords_missing"));
    let report = check(json!({"coordinates": [1.0, 2.0]}));
    assert!(has(&report, "geojson.type_missing"));
    let report = check(json!({"type": "Circle", "coordinates": [1.0, 2.0]}));
    assert!(has(&report, "geojson.type_unknown"));
    let report = check(json!([1, 2]));
    assert!(has(&report, "geojson.root_object"));
}

#[test]
fn feature_members_are_required_but_null_is_fine() {
    let report =
        check(json!({"type": "Feature", "geometry": {"type": "Point", "coordinates": [1.0, 2.0]}}));
    assert!(has(&report, "geojson.feature_members"));
    let report = check(
        json!({"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [1.0, 2.0], "crs": {"type": "name", "properties": {"name": "EPSG:3857"}}}}),
    );
    assert!(has(&report, "geojson.legacy_crs"));
    let report = check(json!({"type": "Feature", "properties": "text", "geometry": null}));
    assert!(has(&report, "geojson.properties_object"));
    // geometry: null / properties: null 合法
    let report = check(json!({"type": "Feature", "properties": null, "geometry": null}));
    assert_eq!(report.overall, "passed");
}

#[test]
fn feature_collection_requires_features_array() {
    let report = check(json!({"type": "FeatureCollection"}));
    assert!(has(&report, "geojson.features_missing"));
    let report = check(json!({"type": "FeatureCollection", "features": {"type": "Feature"}}));
    assert!(has(&report, "geojson.features_missing"));
    let report = check(json!({"type": "FeatureCollection", "features": []}));
    assert_eq!(report.overall, "passed");
    let report =
        check(json!({"type": "FeatureCollection", "features": [{"type": "GeometryCollection"}]}));
    assert!(has(&report, "geojson.not_feature"));
}

#[test]
fn position_dimension_number_and_range_checks() {
    let report = check(json!({"type": "Point", "coordinates": [1.0]}));
    assert!(has(&report, "geojson.position_dims"));
    let report = check(json!({"type": "Point", "coordinates": ["1", 2.0]}));
    assert!(has(&report, "geojson.position_number"));
    let report = check(json!({"type": "Point", "coordinates": [181.0, 2.0]}));
    assert!(has(&report, "geojson.lon_range"));
    let report = check(json!({"type": "Point", "coordinates": [1.0, 91.0]}));
    assert!(has(&report, "geojson.lat_range"));
}

#[test]
fn swapped_lon_lat_is_a_hint_not_a_swap() {
    let report = check(json!({"type": "Point", "coordinates": [55.2744, 25.1972]}));
    assert_eq!(report.overall, "passed");
    // 纬度 121 超范围、经度 31 在范围内：提示疑似反序但不交换
    let report = check(json!({"type": "Point", "coordinates": [31.2304, 121.4737]}));
    assert!(has(&report, "geojson.lat_range"));
    assert!(has(&report, "geojson.maybe_swapped"));
    // 反序之后仍有值越界时提示也可能继续给出，但数值绝不修改
    let point = report
        .diagnostics
        .iter()
        .find(|d| d.code == "geojson.maybe_swapped")
        .unwrap();
    assert!(point.line.is_some(), "启发式提示要带原文位置");
}

#[test]
fn line_and_polygon_structure_checks() {
    let report = check(json!({"type": "LineString", "coordinates": [[1.0, 2.0]]}));
    assert!(has(&report, "geojson.line_too_short"));
    let report = check(json!({"type": "MultiLineString", "coordinates": [[[1.0, 2.0]]]}));
    assert!(has(&report, "geojson.line_too_short"));
    // 未闭合环
    let report = check(
        json!({"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]]]}),
    );
    assert!(has(&report, "geojson.ring_not_closed"));
    // 少于 4 个位置
    let report =
        check(json!({"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 0.0]]]}));
    assert!(has(&report, "geojson.ring_too_short"));
    // 空坐标按兼容策略告警
    let report = check(json!({"type": "Polygon", "coordinates": []}));
    assert!(has(&report, "geojson.empty_coords"));
    assert_eq!(report.overall, "warnings");
}

#[test]
fn ring_winding_is_only_a_hint() {
    // 顺时针外环：右手规则建议逆时针 → 启发式提示，不是错误
    let cw = json!({"type": "Polygon", "coordinates": [[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]]});
    let report = check(cw);
    assert!(has(&report, "geojson.ring_winding"));
    assert_eq!(report.overall, "warnings");
    // 逆时针外环 + 顺时针内环：方向符合建议，整体通过
    let report = check(json!({"type": "Polygon", "coordinates": [
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]],
        [[0.2, 0.2], [0.2, 0.8], [0.8, 0.8], [0.8, 0.2], [0.2, 0.2]]
    ]}));
    assert_eq!(report.overall, "passed");
    // 反方向环只给提示，不改变整体通过状态（hint 不算失败）
    assert_eq!(
        check(json!({"type": "LineString", "coordinates": [[1.0, 2.0], [3.0, 4.0]]})).overall,
        "passed"
    );
}

#[test]
fn elevation_and_extra_dimensions() {
    // 第三位是高程，规范允许
    let report = check(json!({"type": "Point", "coordinates": [1.0, 2.0, 30.5]}));
    assert_eq!(report.overall, "passed");
    // 超过三维是兼容性告警
    let report = check(json!({"type": "Point", "coordinates": [1.0, 2.0, 3.0, 4.0]}));
    assert!(has(&report, "geojson.position_extra"));
}

#[test]
fn bbox_checks_include_antimeridian_crossing() {
    // 西界 > 东界：合法的反经线跨越 bbox
    let report = check(json!({
        "type": "FeatureCollection",
        "features": [{"type": "Feature", "properties": null, "geometry": {"type": "Point", "coordinates": [179.5, 1.0]}}],
        "bbox": [175.0, 0.0, -175.0, 2.0]
    }));
    assert_eq!(report.overall, "passed");
    // 维数错误
    let report = check(json!({"type": "Point", "coordinates": [1.0, 2.0], "bbox": [1.0, 2.0]}));
    assert!(has(&report, "geojson.bbox_dims"));
    // 范围错误
    let report =
        check(json!({"type": "Point", "coordinates": [1.0, 2.0], "bbox": [1.0, 91.0, 2.0, 3.0]}));
    assert!(has(&report, "geojson.bbox_range"));
    // 包含性：点在 bbox 外
    let report = check(json!({
        "type": "Point", "coordinates": [5.0, 5.0], "bbox": [0.0, 0.0, 1.0, 1.0]
    }));
    assert!(has(&report, "geojson.bbox_mismatch"));
}

#[test]
fn unknown_members_are_preserved() {
    let value = json!({
        "type": "Feature",
        "properties": null,
        "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
        "extra": {"anything": true}
    });
    let report = check(value);
    assert_eq!(report.overall, "passed");
}

#[test]
fn limits_mark_report_incomplete() {
    // 坐标数超限：报告必须标记未完成，不能显示「全部通过」
    let mut coords = Vec::new();
    for index in 0..(MAX_POSITIONS as i64 + 10) {
        coords.push(json!([index as f64, 0.0]));
    }
    let report = check(json!({"type": "MultiPoint", "coordinates": coords}));
    assert!(report.incomplete);
    assert_eq!(report.overall, "incomplete");
}

#[test]
fn per_code_limit_truncates_repeated_diagnostics() {
    let mut coords = Vec::new();
    for index in 0..100i64 {
        coords.push(json!(["x", index]));
    }
    let report = check(json!({"type": "MultiPoint", "coordinates": coords}));
    let count = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "geojson.position_number")
        .count();
    assert!(count <= 20);
    assert!(report.stats.errors >= 100);
}
