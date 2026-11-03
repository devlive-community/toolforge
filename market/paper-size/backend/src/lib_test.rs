use super::*;
use serde_json::json;

#[test]
fn lists_all_and_by_query() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, 37);
    assert!(out.sizes.iter().all(|s| s.long > s.short));

    let out = list(ListArgs { query: "a4".into() }).unwrap();
    assert_eq!(out.total, 1);
    assert_eq!(out.sizes[0].key, "a4");
    assert_eq!(out.sizes[0].short, 210.0);
    // C5 纸张与 C5 信封同尺寸
    assert_eq!(list(ListArgs { query: "c5".into() }).unwrap().total, 2);

    let out = list(ListArgs {
        query: "photo".into(),
    })
    .unwrap();
    assert_eq!(out.total, 6, "photo category matches by key prefix");

    assert!(
        list(ListArgs {
            query: "不存在的".into()
        })
        .is_err()
    );
}

#[test]
fn a_series_folds_consistently() {
    let size_of = |key: &str| {
        list(ListArgs { query: key.into() })
            .unwrap()
            .sizes
            .remove(0)
    };
    // ISO 216 √2 比例：A(n) 短边 ≈ A(n-1) 长边的一半（A4 对折是 A5）
    assert!((size_of("a4").short * 2.0 - size_of("a3").long).abs() < 1.0);
    assert!(
        (size_of("a5").short * 2.0 - size_of("a4").long).abs() < 1.5,
        "rounded ISO values"
    );
    assert!((size_of("a4").long - 297.0).abs() < 0.01);
}

#[test]
fn mm_to_pixels_at_dpi() {
    let out = convert(ConvertArgs {
        mm: 210.0,
        dpi: Some(300.0),
    })
    .unwrap();
    assert!((out.inches - 210.0 / 25.4).abs() < 1e-9);
    assert_eq!(out.pixels, 2480, "A4 short side at 300 DPI");
    let out = convert(ConvertArgs {
        mm: 25.4,
        dpi: None,
    })
    .unwrap();
    assert_eq!(out.pixels, 300, "1 inch at the default 300 DPI");
    let err = convert(ConvertArgs { mm: 0.0, dpi: None }).unwrap_err();
    assert_eq!(err.code, "paper.positive_required");
}

#[test]
fn dispatches_via_plugin() {
    let tool = PaperSize::default();
    // C5 纸张与 C5 信封同尺寸：两条都命中
    let out = tool.call("list", json!({"query": "c5"})).unwrap();
    assert_eq!(out["total"], 2);
    let out = tool
        .call("convert", json!({"mm": 25.4, "dpi": 600}))
        .unwrap();
    assert_eq!(out["pixels"], 600);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&PaperSize::default()).unwrap();
    assert_eq!(checked, 2);
}
