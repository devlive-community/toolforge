use super::palette::{Args, palette};

#[test]
fn returns_all_groups_without_query() {
    let out = palette(Args::default()).unwrap();
    assert_eq!(out.groups.len(), 8, "every category has swatches");
    assert_eq!(out.total, 80);
    assert!(out.groups.windows(2).all(|w| w[0].category != w[1].category));
}

#[test]
fn filters_by_name_pinyin_and_hex() {
    let by_name = palette(Args { query: "黛".into() }).unwrap();
    assert!(by_name.total >= 2, "黛蓝 and 黛紫 hit");
    assert!(by_name.groups.iter().all(|g| g.colors.iter().all(|s| s.name.contains('黛') || s.pinyin.contains("黛"))));

    let by_pinyin = palette(Args { query: "dian".into() }).unwrap();
    assert!(by_pinyin.groups.iter().any(|g| g.colors.iter().any(|s| s.pinyin == "dianqing")));

    let by_hex = palette(Args { query: "#ff461f".into() }).unwrap();
    assert_eq!(by_hex.total, 1);
    assert_eq!(by_hex.groups[0].colors[0].name, "朱砂");

    assert!(palette(Args { query: "无此色".into() }).is_err());
}

#[test]
fn hexes_are_six_digit() {
    let out = palette(Args::default()).unwrap();
    for group in &out.groups {
        for swatch in &group.colors {
            assert_eq!(swatch.hex.len(), 7, "{}", swatch.hex);
            assert!(swatch.hex.starts_with('#'));
            assert!(swatch.hex[1..].bytes().all(|b| b.is_ascii_hexdigit()));
        }
    }
}
