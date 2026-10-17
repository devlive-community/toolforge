//! 把表格的列组合成绘图模型：X 轴类型、左右 Y 轴范围与刻度、各系列的点（折线降采样、散点抽样）。
//! 前端只负责把这些值映射到像素并绘制。

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::axis::{self, Scale};
use crate::infer::{self, Kind};
use crate::table::{Note, Table};

/// 折线每个系列最多保留的点数（LTTB 降采样）
pub const MAX_LINE_POINTS: usize = 2000;
/// 散点每个系列最多保留的点数（等间隔抽样）
pub const MAX_SCATTER_POINTS: usize = 5000;
/// 柱状图最多的类目数
pub const MAX_BARS: usize = 500;
/// 最多同时绘制的系列数
pub const MAX_SERIES: usize = 12;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Chart {
    Line,
    Bar,
    Scatter,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum XType {
    #[default]
    Auto,
    Number,
    Time,
    Category,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeriesSpec {
    pub column: usize,
    #[serde(default)]
    pub axis: Side,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotArgs {
    pub token: u64,
    pub chart: Chart,
    /// X 轴列；为空时用行号
    #[serde(default)]
    pub x: Option<usize>,
    #[serde(default)]
    pub x_type: XType,
    pub series: Vec<SeriesSpec>,
    #[serde(default)]
    pub log_left: bool,
    #[serde(default)]
    pub log_right: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum XKind {
    Index,
    Number,
    Time,
    Category,
}

#[derive(Debug, Serialize)]
pub struct XAxis {
    pub kind: XKind,
    pub name: String,
    #[serde(flatten)]
    pub scale: Scale,
}

#[derive(Debug, Serialize)]
pub struct YAxis {
    pub log: bool,
    pub names: Vec<String>,
    #[serde(flatten)]
    pub scale: Scale,
}

#[derive(Debug, Serialize)]
pub struct Series {
    pub name: String,
    pub column: usize,
    pub axis: Side,
    pub points: Vec<[f64; 2]>,
    /// 参与绘制的点数（降采样前）
    pub total: usize,
    /// 因缺值、无法解析或对数轴上非正数而跳过的行
    pub skipped: usize,
    pub sampled: bool,
}

#[derive(Debug, Serialize)]
pub struct Plot {
    pub chart: Chart,
    pub x: XAxis,
    pub left: Option<YAxis>,
    pub right: Option<YAxis>,
    pub series: Vec<Series>,
    /// 类目轴的全部标签（按点的 x 下标取用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    pub notes: Vec<Note>,
}

/// X 轴：类型、列名、每行的值，以及类目轴的标签
type XValues = (XKind, String, Vec<Option<f64>>, Option<Vec<String>>);

fn x_values(table: &Table, args: &PlotArgs, rows: usize) -> PluginResult<XValues> {
    let Some(index) = args.x else {
        return Ok((
            XKind::Index,
            String::new(),
            (1..=rows).map(|i| Some(i as f64)).collect(),
            None,
        ));
    };
    let column = table
        .columns
        .get(index)
        .ok_or_else(|| PluginError::new("plot.unknown_column"))?;
    let name = column.name.clone();
    let category = || {
        // 时间列用简短的本地时间标签，其余用原文
        let labels: Vec<String> = if column.kind == Kind::Time {
            axis::time_labels(&column.values[..rows])
        } else {
            column.raw[..rows].to_vec()
        };
        (
            XKind::Category,
            name.clone(),
            (0..rows).map(|i| Some(i as f64)).collect(),
            Some(labels),
        )
    };
    if args.chart == Chart::Bar {
        return Ok(category());
    }
    Ok(match (args.x_type, column.kind) {
        (XType::Category, _) | (XType::Auto, Kind::Text) => category(),
        (XType::Auto | XType::Time, Kind::Time) => {
            (XKind::Time, name, column.values[..rows].to_vec(), None)
        }
        (XType::Time, Kind::Number) => {
            let values = column.values[..rows]
                .iter()
                .map(|v| v.and_then(infer::epoch_ms))
                .collect();
            (XKind::Time, name, values, None)
        }
        (XType::Time, Kind::Text) => {
            let values = column.raw[..rows]
                .iter()
                .map(|c| infer::datetime(c))
                .collect();
            (XKind::Time, name, values, None)
        }
        (XType::Number, Kind::Text) => {
            let values = column.raw[..rows]
                .iter()
                .map(|c| infer::number(c))
                .collect();
            (XKind::Number, name, values, None)
        }
        // 时间列按数值使用时就是毫秒时间戳
        (XType::Auto | XType::Number, _) => {
            (XKind::Number, name, column.values[..rows].to_vec(), None)
        }
    })
}

/// Largest-Triangle-Three-Buckets：保留折线形状的降采样
pub fn lttb(points: &[[f64; 2]], threshold: usize) -> Vec<[f64; 2]> {
    if threshold >= points.len() || threshold < 3 {
        return points.to_vec();
    }
    let every = (points.len() - 2) as f64 / (threshold - 2) as f64;
    let mut out = Vec::with_capacity(threshold);
    out.push(points[0]);
    let mut a = 0usize;
    for i in 0..threshold - 2 {
        let next_start = ((i + 1) as f64 * every) as usize + 1;
        let next_end = (((i + 2) as f64 * every) as usize + 1).min(points.len());
        let range = &points[next_start..next_end.max(next_start + 1).min(points.len())];
        let (avg_x, avg_y) = range
            .iter()
            .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
        let (avg_x, avg_y) = (avg_x / range.len() as f64, avg_y / range.len() as f64);
        let start = (i as f64 * every) as usize + 1;
        let end = next_start.min(points.len() - 1);
        let pa = points[a];
        let mut best = start;
        let mut best_area = -1.0;
        for (j, p) in points.iter().enumerate().take(end).skip(start) {
            let area = ((pa[0] - avg_x) * (p[1] - pa[1]) - (pa[0] - p[0]) * (avg_y - pa[1])).abs();
            if area > best_area {
                best_area = area;
                best = j;
            }
        }
        out.push(points[best]);
        a = best;
    }
    out.push(points[points.len() - 1]);
    out
}

/// 等间隔抽样，保留首尾
fn stride(points: &[[f64; 2]], max: usize) -> Vec<[f64; 2]> {
    if points.len() <= max {
        return points.to_vec();
    }
    let step = points.len() as f64 / (max - 1) as f64;
    let mut out: Vec<[f64; 2]> = (0..max - 1)
        .map(|i| points[(i as f64 * step) as usize])
        .collect();
    out.push(points[points.len() - 1]);
    out
}

fn y_axis(series: &[Series], side: Side, log: bool, zero: bool) -> Option<YAxis> {
    let on_side: Vec<&Series> = series.iter().filter(|s| s.axis == side).collect();
    if on_side.is_empty() {
        return None;
    }
    let (min, max) = on_side
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p[1]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    let scale = if !min.is_finite() {
        axis::linear(0.0, 1.0, false, false)
    } else if log {
        axis::log(min, max)
    } else {
        axis::linear(min, max, zero, false)
    };
    Some(YAxis {
        log,
        names: on_side.iter().map(|s| s.name.clone()).collect(),
        scale,
    })
}

pub fn build(table: &Table, args: &PlotArgs) -> PluginResult<Plot> {
    if args.series.is_empty() {
        return Err(PluginError::new("plot.no_series"));
    }
    if args.series.len() > MAX_SERIES {
        return Err(PluginError::new("plot.too_many_series").with("max", MAX_SERIES));
    }
    let mut notes = Vec::new();
    let rows = if args.chart == Chart::Bar && table.rows > MAX_BARS {
        notes.push(Note {
            code: "plot.bars_truncated",
            params: json!({ "max": MAX_BARS, "rows": table.rows }),
        });
        MAX_BARS
    } else {
        table.rows
    };
    let (kind, name, xs, categories) = x_values(table, args, rows)?;
    let mut series = Vec::new();
    for spec in &args.series {
        let column = table
            .columns
            .get(spec.column)
            .ok_or_else(|| PluginError::new("plot.unknown_column"))?;
        if column.kind != Kind::Number {
            return Err(
                PluginError::new("plot.series_not_numeric").with("column", column.name.as_str())
            );
        }
        let log = match spec.axis {
            Side::Left => args.log_left,
            Side::Right => args.log_right,
        };
        let mut points = Vec::with_capacity(rows);
        let mut skipped = 0;
        for (x, y) in xs.iter().zip(&column.values[..rows]) {
            match (x, y) {
                (Some(x), Some(y)) if !log || *y > 0.0 => points.push([*x, *y]),
                _ => skipped += 1,
            }
        }
        if args.chart == Chart::Line && matches!(kind, XKind::Number | XKind::Time) {
            points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
        let total = points.len();
        let points = match args.chart {
            Chart::Line => lttb(&points, MAX_LINE_POINTS),
            Chart::Scatter => stride(&points, MAX_SCATTER_POINTS),
            Chart::Bar => points,
        };
        series.push(Series {
            name: column.name.clone(),
            column: spec.column,
            axis: spec.axis,
            sampled: points.len() < total,
            points,
            total,
            skipped,
        });
    }
    let (min, max) = series
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p[0]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    if !min.is_finite() {
        return Err(PluginError::new("plot.no_points"));
    }
    let x_scale = match kind {
        XKind::Index => axis::linear(min, max, false, true),
        XKind::Number => axis::linear(min, max, false, false),
        XKind::Time => axis::time(min, max),
        XKind::Category => axis::categories(categories.as_deref().unwrap_or_default()),
    };
    let zero = args.chart == Chart::Bar;
    let left = y_axis(&series, Side::Left, args.log_left, zero);
    let right = y_axis(&series, Side::Right, args.log_right, zero);
    Ok(Plot {
        chart: args.chart,
        x: XAxis {
            kind,
            name,
            scale: x_scale,
        },
        left,
        right,
        series,
        categories,
        notes,
    })
}

#[cfg(test)]
#[path = "plot_test.rs"]
mod tests;
