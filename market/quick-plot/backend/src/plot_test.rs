use super::*;
use crate::table::{Format, HeaderMode, parse};

fn args(chart: Chart, x: Option<usize>, series: &[(usize, Side)]) -> PlotArgs {
    PlotArgs {
        token: 0,
        chart,
        x,
        x_type: XType::Auto,
        series: series
            .iter()
            .map(|&(column, axis)| SeriesSpec { column, axis })
            .collect(),
        log_left: false,
        log_right: false,
    }
}

fn table(text: &str) -> Table {
    parse(text, Format::Auto, HeaderMode::Auto).unwrap()
}

const BENCH: &str = "time,qps,p99\n\
                     2024-01-01 00:00:00,100,5\n\
                     2024-01-01 00:00:02,300,-1\n\
                     2024-01-01 00:00:01,200,\n";

#[test]
fn builds_time_series_with_two_axes() {
    let plot = build(
        &table(BENCH),
        &args(Chart::Line, Some(0), &[(1, Side::Left), (2, Side::Right)]),
    )
    .unwrap();
    assert_eq!(plot.x.kind, XKind::Time);
    assert_eq!(plot.x.name, "time");
    // 折线按 x 排序
    let ys: Vec<f64> = plot.series[0].points.iter().map(|p| p[1]).collect();
    assert_eq!(ys, [100.0, 200.0, 300.0]);
    assert_eq!((plot.series[1].total, plot.series[1].skipped), (2, 1));
    assert_eq!(plot.left.as_ref().unwrap().names, ["qps"]);
    assert_eq!(plot.right.as_ref().unwrap().names, ["p99"]);
}

#[test]
fn log_axes_skip_non_positive_values() {
    let mut a = args(Chart::Line, Some(0), &[(2, Side::Left)]);
    a.log_left = true;
    let plot = build(&table(BENCH), &a).unwrap();
    assert_eq!((plot.series[0].total, plot.series[0].skipped), (1, 2));
    assert!(plot.left.unwrap().log);
}

#[test]
fn uses_row_numbers_and_categories() {
    let t = table("name,v\na,3\nb,1\nc,2\n");
    let rows = build(&t, &args(Chart::Line, None, &[(1, Side::Left)])).unwrap();
    assert_eq!(rows.x.kind, XKind::Index);
    assert_eq!(rows.series[0].points[0], [1.0, 3.0]);
    let bars = build(&t, &args(Chart::Bar, Some(0), &[(1, Side::Left)])).unwrap();
    assert_eq!(bars.x.kind, XKind::Category);
    assert_eq!(bars.categories.unwrap(), ["a", "b", "c"]);
    // 柱状图的 Y 轴从 0 开始，类目保持原顺序
    assert_eq!(bars.left.unwrap().scale.min, 0.0);
    assert_eq!(bars.series[0].points[0], [0.0, 3.0]);
}

#[test]
fn x_type_overrides_inference() {
    let t = table("ts,v\n1700000000,1\n1700000060,2\n");
    let mut a = args(Chart::Line, Some(0), &[(1, Side::Left)]);
    assert_eq!(build(&t, &a).unwrap().x.kind, XKind::Time);
    a.x_type = XType::Number;
    assert_eq!(build(&t, &a).unwrap().x.kind, XKind::Number);
    a.x_type = XType::Category;
    assert_eq!(build(&t, &a).unwrap().x.kind, XKind::Category);
    let text = table("when,v\nmon,1\n2024-01-01,2\n");
    a.x_type = XType::Time;
    let plot = build(&text, &a).unwrap();
    assert_eq!((plot.x.kind, plot.series[0].skipped), (XKind::Time, 1));
}

#[test]
fn rejects_unusable_selections() {
    let t = table("name,v\na,1\n");
    let code = |a: PlotArgs| build(&t, &a).err().unwrap().code;
    assert_eq!(code(args(Chart::Line, None, &[])), "plot.no_series");
    assert_eq!(
        code(args(Chart::Line, None, &[(0, Side::Left)])),
        "plot.series_not_numeric"
    );
    assert_eq!(
        code(args(Chart::Line, Some(9), &[(1, Side::Left)])),
        "plot.unknown_column"
    );
    let empty = table("a,b\nx,\ny,\n1,\n");
    assert_eq!(
        build(&empty, &args(Chart::Line, None, &[(0, Side::Left)]))
            .err()
            .unwrap()
            .code,
        "plot.series_not_numeric"
    );
}

#[test]
fn downsamples_large_series() {
    let mut text = String::from("x,y,z\n");
    for i in 0..20_000 {
        text.push_str(&format!("{i},{},{}\n", (i as f64 / 100.0).sin(), i % 7));
    }
    let t = table(&text);
    let line = build(&t, &args(Chart::Line, Some(0), &[(1, Side::Left)])).unwrap();
    assert_eq!(line.series[0].points.len(), MAX_LINE_POINTS);
    assert!(line.series[0].sampled);
    assert_eq!(line.series[0].total, 20_000);
    // LTTB 保留首尾与极值
    let points = &line.series[0].points;
    assert_eq!(points[0][0], 0.0);
    assert_eq!(points.last().unwrap()[0], 19_999.0);
    assert!(points.iter().any(|p| p[1] > 0.999));
    let scatter = build(&t, &args(Chart::Scatter, Some(0), &[(2, Side::Left)])).unwrap();
    assert_eq!(scatter.series[0].points.len(), MAX_SCATTER_POINTS);
    let bars = build(&t, &args(Chart::Bar, Some(0), &[(2, Side::Left)])).unwrap();
    assert_eq!(bars.series[0].points.len(), MAX_BARS);
    assert_eq!(bars.notes[0].code, "plot.bars_truncated");
}

#[test]
fn lttb_keeps_short_series() {
    let points = [[0.0, 1.0], [1.0, 2.0]];
    assert_eq!(lttb(&points, 10), points);
    let many: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, (i % 3) as f64]).collect();
    let out = lttb(&many, 5);
    assert_eq!(out.len(), 5);
    assert!(out.windows(2).all(|w| w[0][0] < w[1][0]));
}
