//! RFC 7946 检查：按「规范强制（MUST）→ 兼容性告警（SHOULD/MAY）→ 启发式提示」分级，
//! 每个 diagnostic 携带 JSON Pointer 与原文位置。启发式只提示，不做自动交换或修复。

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::spans::{self, Spans};

/// 诊断级别：error=规范强制，warning=兼容性告警，hint=启发式提示
pub const ERROR: &str = "error";
pub const WARNING: &str = "warning";
pub const HINT: &str = "hint";

/// 诊断数量上限（超出后停止记录并标记未完成）
pub const MAX_DIAGNOSTICS: usize = 1000;
/// 坐标数量上限
pub const MAX_POSITIONS: u64 = 500_000;
/// 同类低价值诊断的逐条记录上限（之后只累计数量）
const PER_CODE_LIMIT: usize = 20;

const GEOMETRY_TYPES: &[&str] = &[
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Diag {
    /// error | warning | hint
    pub level: &'static str,
    pub code: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, Value>,
    /// RFC 6901 JSON Pointer，根为空字符串
    pub pointer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub features: u64,
    pub positions: u64,
    pub types: BTreeMap<String, u64>,
    pub errors: u64,
    pub warnings: u64,
    pub hints: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// passed | invalid | warnings | incomplete
    pub overall: &'static str,
    /// 达到任一上限而未检查完整；未完成时不得展示「全部通过」
    pub incomplete: bool,
    pub stats: Stats,
    pub diagnostics: Vec<Diag>,
}

struct Walker<'a> {
    spans: &'a Spans,
    diags: Vec<Diag>,
    per_code: BTreeMap<String, usize>,
    stats: Stats,
    incomplete: bool,
}

impl<'a> Walker<'a> {
    fn new(spans: &'a Spans) -> Self {
        Self {
            spans,
            diags: Vec::new(),
            per_code: BTreeMap::new(),
            stats: Stats::default(),
            incomplete: false,
        }
    }

    fn push(
        &mut self,
        level: &'static str,
        code: &str,
        pointer: &str,
        params: serde_json::Map<String, Value>,
    ) {
        if self.diags.len() >= MAX_DIAGNOSTICS {
            self.incomplete = true;
            return;
        }
        let seen = self.per_code.entry(code.to_owned()).or_default();
        *seen += 1;
        // 同类问题逐条记录到上限，之后仅在统计里累加，避免淹没输出
        if *seen <= PER_CODE_LIMIT {
            let (line, column) = located(self.spans, pointer);
            self.diags.push(Diag {
                level,
                code: code.to_owned(),
                params,
                pointer: pointer.to_owned(),
                line,
                column,
            });
        } else if *seen == PER_CODE_LIMIT + 1 {
            // 首次截断时插入一条汇总说明
            let (line, column) = located(self.spans, pointer);
            self.diags.push(Diag {
                level: HINT,
                code: "geojson.more_of_code".to_owned(),
                params: serde_json::Map::from_iter([("code".to_owned(), Value::from(code))]),
                pointer: pointer.to_owned(),
                line,
                column,
            });
        }
        match level {
            ERROR => self.stats.errors += 1,
            WARNING => self.stats.warnings += 1,
            _ => self.stats.hints += 1,
        }
    }

    fn diag1(&mut self, level: &'static str, code: &str, pointer: &str) {
        self.push(level, code, pointer, serde_json::Map::new());
    }

    fn diagp(
        &mut self,
        level: &'static str,
        code: &str,
        pointer: &str,
        key: &str,
        value: impl Into<Value>,
    ) {
        self.push(
            level,
            code,
            pointer,
            serde_json::Map::from_iter([(key.to_owned(), value.into())]),
        );
    }
}

fn lon(number: f64) -> bool {
    (-180.0..=180.0).contains(&number)
}

/// pointer 的原文位置（None 表示扫描未覆盖）
fn located(spans: &Spans, pointer: &str) -> (Option<u64>, Option<u64>) {
    match spans::locate(spans, pointer) {
        Some((line, column)) => (Some(line), Some(column)),
        None => (None, None),
    }
}

fn lat(number: f64) -> bool {
    (-90.0..=90.0).contains(&number)
}

/// 校验 Position：维数、数值、经纬度范围与疑似反序
fn check_position(walker: &mut Walker, value: &Value, pointer: &str) {
    walker.stats.positions += 1;
    if walker.stats.positions > MAX_POSITIONS {
        walker.incomplete = true;
        return;
    }
    let Some(items) = value.as_array() else {
        walker.diagp(ERROR, "geojson.position_array", pointer, "pointer", pointer);
        return;
    };
    if items.len() < 2 {
        walker.diag1(ERROR, "geojson.position_dims", pointer);
        return;
    }
    if items.len() > 3 {
        walker.push(
            WARNING,
            "geojson.position_extra",
            pointer,
            serde_json::Map::from_iter([("dims".to_owned(), Value::from(items.len()))]),
        );
    }
    for (index, item) in items.iter().enumerate() {
        let Some(number) = item.as_f64() else {
            walker.diag1(ERROR, "geojson.position_number", pointer);
            return;
        };
        match index {
            0 if !lon(number) => {
                walker.diag1(ERROR, "geojson.lon_range", pointer);
            }
            1 if !lat(number) => {
                walker.diag1(ERROR, "geojson.lat_range", pointer);
                // 疑似反序：第二位超出纬度范围、第一位在经度范围内时提示，
                // 不自动交换，也不推断坐标系
                if let Some(first) = items.first().and_then(Value::as_f64)
                    && lon(first)
                    && number.abs() <= 180.0
                {
                    walker.diag1(HINT, "geojson.maybe_swapped", pointer);
                }
            }
            _ => {}
        }
    }
}

/// 校验 bbox（RFC 7946 §5）：维数、数值、范围与包含性（含反经线跨越）
fn check_bbox(walker: &mut Walker, value: &Value, pointer: &str, coords: Option<&Value>) {
    let Some(items) = value.as_array() else {
        walker.diag1(ERROR, "geojson.bbox_array", pointer);
        return;
    };
    if items.len() != 4 && items.len() != 6 {
        walker.diagp(ERROR, "geojson.bbox_dims", pointer, "dims", items.len());
        return;
    }
    let numbers: Vec<f64> = items
        .iter()
        .map(Value::as_f64)
        .collect::<Option<_>>()
        .unwrap_or_default();
    if numbers.len() != items.len() {
        walker.diag1(ERROR, "geojson.bbox_number", pointer);
        return;
    }
    for (index, number) in numbers.iter().enumerate() {
        let in_range = if index % 2 == 0 {
            lon(*number)
        } else {
            lat(*number)
        };
        if !in_range {
            walker.diag1(ERROR, "geojson.bbox_range", pointer);
            return;
        }
    }
    // 反经线跨越：西界大于东界是合法的，包含性按「lon ≥ 西界 或 lon ≤ 东界」判定，
    // 不能仅因西界大于东界就判错
    let (west, south, east, north) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    let crossing = west > east;
    let Some(coords) = coords else { return };
    let mut outside = 0u64;
    let mut visit = |walker: &mut Walker, position: &Value, pointer: &str| {
        let Some(items) = position.as_array() else {
            return;
        };
        let (Some(lon_value), Some(lat_value)) = (
            items.first().and_then(Value::as_f64),
            items.get(1).and_then(Value::as_f64),
        ) else {
            return;
        };
        let lon_inside = if crossing {
            lon_value >= west || lon_value <= east
        } else {
            lon_value >= west && lon_value <= east
        };
        if !(lon_inside && lat_value >= south && lat_value <= north) {
            outside += 1;
            if outside <= PER_CODE_LIMIT as u64 {
                walker.diag1(WARNING, "geojson.bbox_mismatch", pointer);
            }
        }
    };
    collect_positions(coords, pointer, walker, &mut visit);
    if outside > PER_CODE_LIMIT as u64 {
        walker.push(
            WARNING,
            "geojson.bbox_mismatch_many",
            pointer,
            serde_json::Map::from_iter([("count".to_owned(), Value::from(outside))]),
        );
    }
}

/// 遍历坐标树中的每个 Position
fn collect_positions(
    value: &Value,
    pointer: &str,
    walker: &mut Walker,
    visit: &mut dyn FnMut(&mut Walker, &Value, &str),
) {
    if let Value::Array(items) = value {
        if !items.is_empty() && items.iter().all(Value::is_number) {
            visit(walker, value, pointer);
            return;
        }
        for (index, item) in items.iter().enumerate() {
            let child = format!("{pointer}/{index}");
            collect_positions(item, &child, walker, visit);
            if walker.incomplete {
                return;
            }
        }
    }
}

fn coord(position: &Value) -> Option<(f64, f64)> {
    let items = position.as_array()?;
    Some((items.first()?.as_f64()?, items.get(1)?.as_f64()?))
}

/// 有符号面积（鞋带公式）：正值=逆时针（CCW）
fn ring_winding(ring: &[Value]) -> f64 {
    let mut area = 0.0;
    for pair in ring.windows(2) {
        if let (Some(a), Some(b)) = (coord(&pair[0]), coord(&pair[1])) {
            area += a.0 * b.1 - b.0 * a.1;
        }
    }
    area / 2.0
}

fn check_ring(walker: &mut Walker, ring: &Value, pointer: &str, exterior: bool) {
    let Some(items) = ring.as_array() else {
        walker.diag1(ERROR, "geojson.ring_array", pointer);
        return;
    };
    if items.is_empty() {
        // 空环按空坐标的兼容策略处理
        walker.diag1(WARNING, "geojson.empty_coords", pointer);
        return;
    }
    if items.len() < 4 {
        walker.diag1(ERROR, "geojson.ring_too_short", pointer);
        return;
    }
    for (index, position) in items.iter().enumerate() {
        check_position(walker, position, &format!("{pointer}/{index}"));
        if walker.incomplete {
            return;
        }
    }
    // 首尾闭合（MUST）
    if items.first() != items.last() {
        walker.diag1(ERROR, "geojson.ring_not_closed", pointer);
        return;
    }
    // 环方向：RFC 7946 建议右手规则（外环逆时针、内环顺时针），解析器不应拒绝，
    // 因此只给启发式提示；跨反经线的多边形方向可能反转，以说明为主
    let area = ring_winding(items);
    if (exterior && area < 0.0) || (!exterior && area > 0.0) {
        walker.diag1(HINT, "geojson.ring_winding", pointer);
    }
}

fn check_geometry_coordinates(walker: &mut Walker, kind: &str, coords: &Value, pointer: &str) {
    let Some(items) = coords.as_array() else {
        walker.diagp(ERROR, "geojson.coords_array", pointer, "kind", kind);
        return;
    };
    match kind {
        "Point" => check_position(walker, coords, pointer),
        "MultiPoint" | "LineString" => {
            if items.is_empty() {
                walker.diag1(WARNING, "geojson.empty_coords", pointer);
                return;
            }
            for (index, position) in items.iter().enumerate() {
                check_position(walker, position, &format!("{pointer}/{index}"));
                if walker.incomplete {
                    return;
                }
            }
            if kind == "LineString" && items.len() < 2 {
                walker.diag1(ERROR, "geojson.line_too_short", pointer);
            }
        }
        "MultiLineString" => {
            if items.is_empty() {
                walker.diag1(WARNING, "geojson.empty_coords", pointer);
                return;
            }
            for (index, line) in items.iter().enumerate() {
                let child = format!("{pointer}/{index}");
                let Some(positions) = line.as_array() else {
                    walker.diagp(ERROR, "geojson.coords_array", &child, "kind", "LineString");
                    continue;
                };
                for (at, position) in positions.iter().enumerate() {
                    check_position(walker, position, &format!("{child}/{at}"));
                    if walker.incomplete {
                        return;
                    }
                }
                if positions.len() < 2 {
                    walker.diag1(ERROR, "geojson.line_too_short", &child);
                }
            }
        }
        "Polygon" => {
            if items.is_empty() {
                walker.diag1(WARNING, "geojson.empty_coords", pointer);
                return;
            }
            for (index, ring) in items.iter().enumerate() {
                check_ring(walker, ring, &format!("{pointer}/{index}"), index == 0);
                if walker.incomplete {
                    return;
                }
            }
        }
        "MultiPolygon" => {
            if items.is_empty() {
                walker.diag1(WARNING, "geojson.empty_coords", pointer);
                return;
            }
            for (index, polygon) in items.iter().enumerate() {
                let child = format!("{pointer}/{index}");
                let Some(rings) = polygon.as_array() else {
                    walker.diagp(ERROR, "geojson.coords_array", &child, "kind", "Polygon");
                    continue;
                };
                for (at, ring) in rings.iter().enumerate() {
                    check_ring(walker, ring, &format!("{child}/{at}"), at == 0);
                    if walker.incomplete {
                        return;
                    }
                }
            }
        }
        _ => {}
    }
}

fn check_geometry(walker: &mut Walker, value: &Value, pointer: &str) {
    let Some(object) = value.as_object() else {
        walker.diag1(ERROR, "geojson.geometry_object", pointer);
        return;
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        walker.diag1(ERROR, "geojson.type_missing", pointer);
        return;
    };
    *walker.stats.types.entry(kind.to_owned()).or_default() += 1;
    match kind {
        "GeometryCollection" => {
            // GeometryCollection 没有 coordinates 成员
            match object.get("geometries") {
                Some(Value::Array(geometries)) => {
                    if geometries.is_empty() {
                        walker.diag1(WARNING, "geojson.empty_coords", pointer);
                    }
                    for (index, geometry) in geometries.iter().enumerate() {
                        let child = format!("{pointer}/geometries/{index}");
                        if geometry.is_null() {
                            walker.diag1(WARNING, "geojson.null_geometry", &child);
                            continue;
                        }
                        check_geometry(walker, geometry, &child);
                        if walker.incomplete {
                            return;
                        }
                    }
                }
                _ => walker.diag1(ERROR, "geojson.geometries_missing", pointer),
            }
            if let Some(bbox) = object.get("bbox") {
                check_bbox(walker, bbox, &format!("{pointer}/bbox"), None);
            }
            check_crs(walker, object, pointer);
        }
        other if GEOMETRY_TYPES.contains(&other) => {
            match object.get("coordinates") {
                Some(coords) => {
                    check_geometry_coordinates(
                        walker,
                        other,
                        coords,
                        &format!("{pointer}/coordinates"),
                    );
                    if !walker.incomplete
                        && let Some(bbox) = object.get("bbox")
                    {
                        check_bbox(walker, bbox, &format!("{pointer}/bbox"), Some(coords));
                    }
                }
                None => walker.diag1(ERROR, "geojson.coords_missing", pointer),
            }
            check_crs(walker, object, pointer);
        }
        _ => walker.diagp(ERROR, "geojson.type_unknown", pointer, "type", kind),
    }
}

/// 旧式 CRS 声明：RFC 7946 已移除，只提示不拒绝；null 视为「无 CRS」合法
fn check_crs(walker: &mut Walker, object: &serde_json::Map<String, Value>, pointer: &str) {
    match object.get("crs") {
        Some(Value::Null) | None => {}
        Some(_) => walker.diag1(HINT, "geojson.legacy_crs", &format!("{pointer}/crs")),
    }
}

fn check_feature(walker: &mut Walker, value: &Value, pointer: &str) {
    let Some(object) = value.as_object() else {
        walker.diag1(ERROR, "geojson.not_feature", pointer);
        return;
    };
    if object.get("type").and_then(Value::as_str) != Some("Feature") {
        walker.diag1(ERROR, "geojson.not_feature", pointer);
        return;
    }
    walker.stats.features += 1;
    // geometry 与 properties 是规定成员：缺失报错，null 合法
    match object.get("geometry") {
        Some(Value::Null) => {}
        Some(geometry) => {
            check_geometry(walker, geometry, &format!("{pointer}/geometry"));
            if walker.incomplete {
                return;
            }
        }
        None => walker.diag1(ERROR, "geojson.feature_members", pointer),
    }
    match object.get("properties") {
        None => walker.diag1(ERROR, "geojson.feature_members", pointer),
        Some(Value::Null) | Some(Value::Object(_)) => {}
        Some(_) => walker.diag1(
            ERROR,
            "geojson.properties_object",
            &format!("{pointer}/properties"),
        ),
    }
    if let Some(bbox) = object.get("bbox") {
        check_bbox(walker, bbox, &format!("{pointer}/bbox"), None);
    }
}

fn check_root(walker: &mut Walker, value: &Value, pointer: &str) {
    let Some(object) = value.as_object() else {
        walker.diag1(ERROR, "geojson.root_object", pointer);
        return;
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        walker.diag1(ERROR, "geojson.type_missing", pointer);
        return;
    };
    match kind {
        "FeatureCollection" => match object.get("features") {
            Some(Value::Array(features)) => {
                for (index, feature) in features.iter().enumerate() {
                    check_feature(walker, feature, &format!("{pointer}/features/{index}"));
                    if walker.incomplete {
                        return;
                    }
                }
            }
            _ => walker.diag1(ERROR, "geojson.features_missing", pointer),
        },
        "Feature" => check_feature(walker, value, pointer),
        other if GEOMETRY_TYPES.contains(&other) => check_geometry(walker, value, pointer),
        _ => walker.diagp(ERROR, "geojson.type_unknown", pointer, "type", kind),
    }
    if let Some(bbox) = object.get("bbox") {
        check_bbox(walker, bbox, &format!("{pointer}/bbox"), None);
    }
}

/// 入口：语法已由 serde_json 校验，这里只做 GeoJSON 语义检查
pub fn run(value: &Value, spans: &Spans) -> Report {
    let mut walker = Walker::new(spans);
    check_root(&mut walker, value, "");
    let overall = if walker.incomplete {
        "incomplete"
    } else if walker.stats.errors > 0 {
        "invalid"
    } else if walker.stats.warnings > 0 || walker.stats.hints > 0 {
        // 兼容性告警与启发式提示都是非阻塞发现
        "warnings"
    } else {
        "passed"
    };
    Report {
        overall,
        incomplete: walker.incomplete,
        stats: walker.stats,
        diagnostics: walker.diags,
    }
}

#[cfg(test)]
#[path = "rules_test.rs"]
mod tests;
